#![allow(dead_code)]
#![feature(
    hash_drain_filter,
    int_roundings,
    result_option_inspect,
    fn_traits,
    drain_filter,
    let_chains
)]
mod game;
mod glwrappers;
mod gui;
mod model;
mod util;

use std::cell::RefCell;
use std::collections::HashMap;
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use discord_rich_presence::activity::Activity;
use discord_rich_presence::activity::Assets;
use discord_rich_presence::activity::Timestamps;
use discord_rich_presence::DiscordIpc;
use discord_rich_presence::DiscordIpcClient;
use game::AtlassedTexture;
use game::Block;
use game::BlockRegistry;
use game::BlockRenderType;
use game::Entity;
use game::SoundManager;
use game::StaticBlockModelConnections;
use glwrappers::Buffer;
use glwrappers::Vertex;
use glwrappers::VertexArray;
use image::DynamicImage;
use image::EncodableLayout;
use image::Rgba;
use image::RgbaImage;
use json::JsonValue;
use model::Model;
use ogl33::c_char;
use ogl33::c_void;
use rustc_hash::FxHashMap;
use rusttype::Glyph;
use rusttype::GlyphId;
use rusttype::Point;
use rusttype::Scale;
use sdl2::image::LoadSurface;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseButton;
use sdl2::surface::Surface;
use sdl2::sys::KeyCode;
use sdl2::video::SwapInterval;
use texture_packer::{exporter::ImageExporter, importer::ImageImporter, texture::Texture};
use tungstenite::WebSocket;
use ultraviolet::Mat4;
use ultraviolet::Vec3;
use util::*;

use endio::BERead;

use sdl2::event::*;

fn main() {
    let mut args = std::env::args();
    args.next();

    let sdl = sdl2::init().unwrap();
    let video_subsystem = sdl.video().unwrap();
    let window = RefCell::new(
        video_subsystem
            .window("Game", 900, 700)
            .opengl()
            .fullscreen_desktop()
            .resizable()
            .build()
            .unwrap(),
    );
    let _gl_context = { window.borrow().gl_create_context().unwrap() }; //do not drop

    let (mut win_width, mut win_height) = { window.borrow().size() };
    let mut last_time = 0f32;
    let mut keys_held: std::collections::HashSet<sdl2::keyboard::Keycode> =
        std::collections::HashSet::new();
    unsafe {
        ogl33::load_gl_with(|f_name| {
            sdl2::sys::SDL_GL_GetProcAddress(f_name as *const c_char) as *const c_void
        });
        ogl33::glEnable(ogl33::GL_DEPTH_TEST);
        //ogl33::glEnable(ogl33::GL_CULL_FACE);
        ogl33::glFrontFace(ogl33::GL_CW);
        ogl33::glCullFace(ogl33::GL_BACK);
        ogl33::glClearColor(0.2, 0.3, 0.3, 1.0);
        ogl33::glViewport(0, 0, win_width as i32, win_height as i32)
    }
    let mut sound_manager = SoundManager::new();
    let mut assets = std::path::Path::new(args.next().unwrap().as_str()).to_path_buf();
    assets.push("icon.png");
    {
        window
            .borrow_mut()
            .set_icon(Surface::from_file(assets.as_os_str()).unwrap());
    }
    assets.pop();
    let (texture_atlas, packed_texture, font) = {
        let mut textures_to_pack = Vec::new();
        for asset in std::fs::read_dir(&assets).unwrap() {
            let asset = asset.unwrap();
            let name = asset.file_name();
            if name.to_str().unwrap().ends_with(".png") {
                textures_to_pack.push((
                    name.to_str().unwrap().replace(".png", ""),
                    asset.path().to_path_buf(),
                ));
            }
            if name.to_str().unwrap().ends_with(".wav") {
                sound_manager.load(
                    name.to_str().unwrap().replace(".wav", ""),
                    asset.path().as_path(),
                );
            }
        }
        let font = {
            assets.push("font.ttf");
            let font = rusttype::Font::try_from_vec(std::fs::read(&assets).unwrap()).unwrap();
            assets.pop();
            font
        };
        let ret = pack_textures(textures_to_pack, &font);
        (ret.0, ret.1, font)
    };
    let (block_registry, entity_registry, item_registry) = {
        assets.push("content.json");
        let content = json::parse(std::fs::read_to_string(&assets).unwrap().as_str()).unwrap();
        assets.pop();

        let mut block_registry = BlockRegistry { blocks: Vec::new() };
        block_registry.blocks.insert(0, Block::new_air());
        block_registry
            .blocks
            .resize(content["blocks"].len() + 1, Block::new_air());
        for block in content["blocks"].members() {
            let id = block["id"].as_u32().unwrap();
            let model = &block["model"];
            match model["type"].as_str().unwrap() {
                "cube" => {
                    block_registry.blocks[id as usize] = game::Block {
                        render_data: model["render_data"].as_u8().unwrap_or(0),
                        render_type: game::BlockRenderType::Cube(
                            model["transparent"].as_bool().unwrap_or(false),
                            texture_atlas.get(model["north"].as_str().unwrap()).clone(),
                            texture_atlas.get(model["south"].as_str().unwrap()).clone(),
                            texture_atlas.get(model["right"].as_str().unwrap()).clone(),
                            texture_atlas.get(model["left"].as_str().unwrap()).clone(),
                            texture_atlas.get(model["up"].as_str().unwrap()).clone(),
                            texture_atlas.get(model["down"].as_str().unwrap()).clone(),
                        ),
                        fluid: model["fluid"].as_bool().unwrap_or(false),
                        no_collision: model["no_collide"].as_bool().unwrap_or(false),
                        light: {
                            let light = &model["light"];
                            if !light.is_null() {
                                (
                                    light[0].as_u8().unwrap().min(15),
                                    light[1].as_u8().unwrap().min(15),
                                    light[2].as_u8().unwrap().min(15),
                                )
                            } else {
                                (0, 0, 0)
                            }
                        },
                    };
                }
                "static" => {
                    let texture = texture_atlas
                        .get(model["texture"].as_str().unwrap())
                        .clone();
                    /*let bb_model = &model["model"];
                    let models = if bb_model.is_array() {
                        let mut models = Vec::new();
                        for model in bb_model.members() {
                            models.push(
                                json::parse(
                                    {
                                        assets
                                            .push(model.as_str().unwrap().to_string() + ".bbmodel");
                                        let json = match std::fs::read_to_string(&assets) {
                                            Ok(string) => string,
                                            Err(_) => {
                                                include_str!("missing_block.bbmodel").to_string()
                                            }
                                        };
                                        assets.pop();
                                        json
                                    }
                                    .as_str(),
                                )
                                .unwrap(),
                            );
                        }
                        models
                    } else {
                        vec![{
                            json::parse(
                                {
                                    assets
                                        .push(bb_model.as_str().unwrap().to_string() + ".bbmodel");
                                    let json = match std::fs::read_to_string(&assets) {
                                        Ok(string) => string,
                                        Err(_) => include_str!("missing_block.bbmodel").to_string(),
                                    };
                                    assets.pop();
                                    json
                                }
                                .as_str(),
                            )
                            .unwrap()
                        }]
                    };*/
                    let mut connections = StaticBlockModelConnections {
                        front: HashMap::new(),
                        back: HashMap::new(),
                        left: HashMap::new(),
                        right: HashMap::new(),
                        up: HashMap::new(),
                        down: HashMap::new(),
                    };
                    /*let connections_json = &model["connections"];
                    if !connections_json.is_null() {
                        for face in Face::all() {
                            let down = &connections_json[face.get_name()];
                            if !down.is_null() {
                                for connection in down.entries() {
                                    let id: u32 = connection.0.parse().unwrap();
                                    connections.by_face_mut(face).insert(
                                        id,
                                        StaticBlockModel::new(
                                            &vec![{
                                                json::parse(
                                                    {
                                                        assets.push(
                                                            connection.1["model"]
                                                                .as_str()
                                                                .unwrap()
                                                                .to_string()
                                                                + ".bbmodel",
                                                        );
                                                        let json = match std::fs::read_to_string(
                                                            &assets,
                                                        ) {
                                                            Ok(string) => string,
                                                            Err(_) => include_str!(
                                                                "missing_block.bbmodel"
                                                            )
                                                            .to_string(),
                                                        };
                                                        assets.pop();
                                                        json
                                                    }
                                                    .as_str(),
                                                )
                                                .unwrap()
                                            }],
                                            texture_atlas
                                                .get(connection.1["texture"].as_str().unwrap()),
                                        ),
                                    );
                                }
                            }
                        }
                    }*/
                    block_registry.blocks[id as usize] = Block {
                        render_data: model["render_data"].as_u8().unwrap_or(0),
                        render_type: BlockRenderType::StaticModel(
                            model["transparent"].as_bool().unwrap_or(false),
                            Model::new(
                                include_bytes!("missing.bbm").to_vec(),
                                texture_atlas.missing_texture.clone(),
                            ),
                            model["north"].as_bool().unwrap_or(false),
                            model["south"].as_bool().unwrap_or(false),
                            model["right"].as_bool().unwrap_or(false),
                            model["left"].as_bool().unwrap_or(false),
                            model["up"].as_bool().unwrap_or(false),
                            model["down"].as_bool().unwrap_or(false),
                            connections,
                            model["foliage"].as_bool().unwrap_or(false),
                        ),
                        fluid: model["fluid"].as_bool().unwrap_or(false),
                        no_collision: model["no_collide"].as_bool().unwrap_or(false),
                        light: {
                            let light = &model["light"];
                            if !light.is_null() {
                                (
                                    light[0].as_u8().unwrap().min(15),
                                    light[1].as_u8().unwrap().min(15),
                                    light[2].as_u8().unwrap().min(15),
                                )
                            } else {
                                (0, 0, 0)
                            }
                        },
                    };
                }
                "foliage" => {
                    block_registry.blocks[id as usize] = Block {
                        render_data: model["render_data"].as_u8().unwrap_or(0),
                        render_type: BlockRenderType::Foliage(
                            model["texture1"]
                                .as_str()
                                .map(|t| texture_atlas.get(t).clone()),
                            model["texture2"]
                                .as_str()
                                .map(|t| texture_atlas.get(t).clone()),
                            model["texture3"]
                                .as_str()
                                .map(|t| texture_atlas.get(t).clone()),
                            model["texture4"]
                                .as_str()
                                .map(|t| texture_atlas.get(t).clone()),
                        ),
                        fluid: model["fluid"].as_bool().unwrap_or(false),
                        no_collision: model["no_collide"].as_bool().unwrap_or(false),
                        light: {
                            let light = &model["light"];
                            if !light.is_null() {
                                (
                                    light[0].as_u8().unwrap().min(15),
                                    light[1].as_u8().unwrap().min(15),
                                    light[2].as_u8().unwrap().min(15),
                                )
                            } else {
                                (0, 0, 0)
                            }
                        },
                    }
                }
                _ => unreachable!(),
            }
        }
        let mut entity_registry: HashMap<u32, (EntityRenderData, model::Model)> = HashMap::new();
        for entity in content["entities"].members() {
            let id = entity["id"].as_u32().unwrap();
            let entity_render_data = EntityRenderData {
                model: entity["model"].as_str().unwrap().to_string(),
                texture: entity["texture"].as_str().unwrap().to_string(),
                hitbox_w: entity["hitboxW"].as_f32().unwrap(),
                hitbox_h: entity["hitboxH"].as_f32().unwrap(),
                hitbox_d: entity["hitboxD"].as_f32().unwrap(),
            };
            assets.push(&entity_render_data.model);
            let model = match std::fs::read(assets.as_path()) {
                Ok(data) => (
                    entity_render_data,
                    Model::new(data, texture_atlas.missing_texture.clone()),
                ),
                Err(_) => (
                    entity_render_data,
                    Model::new(
                        include_bytes!("missing.bbm").to_vec(),
                        texture_atlas.missing_texture.clone(),
                    ),
                ),
            };
            assets.pop();
            entity_registry.insert(id, model);
        }
        let mut item_registry: HashMap<u32, ItemRenderData> = HashMap::new();
        for item in content["items"].members() {
            let id = item["id"].as_u32().unwrap();
            let item_render_data = ItemRenderData {
                name: item["name"].as_str().unwrap().to_string(),
                model: match item["modelType"].as_str().unwrap() {
                    "texture" => {
                        ItemModel::Texture(item["modelValue"].as_str().unwrap().to_string())
                    }
                    "block" => ItemModel::Block(item["modelValue"].as_u32().unwrap()),
                    _ => unreachable!(),
                },
            };
            item_registry.insert(id, item_render_data);
        }
        (block_registry, entity_registry, item_registry)
    };
    let mut camera = game::ClientPlayer::at_position(
        ultraviolet::Vec3 {
            x: 0f32,
            y: 50f32,
            z: 0f32,
        },
        &block_registry,
    );
    let addr = args.next().unwrap();
    let tcp_stream = std::net::TcpStream::connect(&addr).unwrap();
    let (mut socket, _response) = tungstenite::client::client_with_config(
        url::Url::parse("ws://aaa123").unwrap(),
        tcp_stream,
        None,
    )
    .unwrap();
    socket.get_mut().set_nonblocking(true).unwrap();
    let (discord_rpc_thread, discord_rpc_thread_tx) = {
        let (discord_thread_tx, discord_thread_rx) = std::sync::mpsc::channel();
        let discord_thread = std::thread::spawn(move || {
            let mut drpc = DiscordIpcClient::new("1088876238447321128").unwrap();
            match drpc.connect() {
                Ok(_) => {
                    println!("discord rpc started!");
                    drpc.set_activity(
        Activity::new()
            .state(format!("Connected to {}", addr).as_str())
            .assets(Assets::new().large_image("https://cdn.discordapp.com/app-icons/1088876238447321128/8e9d838b6ccc9010f6e762023127f1c8.png?size=128")).timestamps(Timestamps::new().start(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64)),
    )
    .expect("Failed to set activity");
                    discord_thread_rx.recv().unwrap();
                    drpc.close().unwrap();
                }
                Err(_) => {}
            };
        });
        (discord_thread, discord_thread_tx)
    };

    let chunk_shader = glwrappers::Shader::new(
        include_str!("shaders/chunk.vert").to_string(),
        include_str!("shaders/chunk.frag").to_string(),
    );
    let outline_shader = glwrappers::Shader::new(
        include_str!("shaders/outline.vert").to_string(),
        include_str!("shaders/outline.frag").to_string(),
    );
    let model_shader = glwrappers::Shader::new(
        include_str!("shaders/model.vert").to_string(),
        include_str!("shaders/model.frag").to_string(),
    );
    let gui_shader = glwrappers::Shader::new(
        include_str!("shaders/gui.vert").to_string(),
        include_str!("shaders/gui.frag").to_string(),
    );
    let texture = glwrappers::Texture::new(
        packed_texture.as_bytes().to_vec(),
        packed_texture.width(),
        packed_texture.height(),
    )
    .expect("couldnt load image");
    texture.bind();
    video_subsystem
        .gl_set_swap_interval(SwapInterval::VSync)
        .unwrap();
    let mut outline_renderer = BlockOutline::new();
    let mut world = game::World::new(&block_registry);
    let mut event_pump = sdl.event_pump().unwrap();
    let timer = sdl.timer().unwrap();
    let mut gui = gui::GUI::new(
        gui::TextRenderer {
            font,
            texture_atlas: &texture_atlas,
        },
        &item_registry,
        texture_atlas.clone(),
        &sdl,
        (win_width, win_height),
        &window,
        &block_registry,
    );
    let win_id = { window.borrow().id() };
    let mut block_breaking_manager = BlockBreakingManager::new(vec![
        texture_atlas.get("breaking1").clone(),
        texture_atlas.get("breaking2").clone(),
        texture_atlas.get("breaking3").clone(),
        texture_atlas.get("breaking4").clone(),
        texture_atlas.get("breaking5").clone(),
        texture_atlas.get("breaking6").clone(),
        texture_atlas.get("breaking7").clone(),
        texture_atlas.get("breaking8").clone(),
        texture_atlas.get("breaking9").clone(),
    ]);
    let mut fps = 0u32;
    let mut last_fps_cnt = 0u32;
    let mut last_fps_time = timer.ticks();
    let mut entities: HashMap<u32, game::Entity> = HashMap::new();
    let mut world_entity_renderer = WorldEntityRenderer::new();
    let mut sky_renderer = SkyRenderer::new();
    let mut fluid_selectable = false;
    let mut particle_manager = game::ParticleManager::new();
    'main_loop: loop {
        'message_loop: loop {
            match socket.read_message() {
                Ok(msg) => match msg {
                    tungstenite::Message::Binary(msg) => {
                        let msg = msg.as_slice();
                        let message = NetworkMessageS2C::from_data(msg).unwrap();
                        match message {
                            NetworkMessageS2C::SetBlock(x, y, z, id) => {
                                let position = BlockPosition { x, y, z };
                                world
                                    .set_block(position, id)
                                    .expect(format!("chunk not loaded at {x} {y} {z}").as_str());
                            }
                            NetworkMessageS2C::LoadChunk(x, y, z, blocks) => {
                                let mut decoder =
                                    libflate::zlib::Decoder::new(blocks.as_slice()).unwrap();
                                let mut blocks_data = Vec::new();
                                std::io::copy(&mut decoder, &mut blocks_data).unwrap();
                                let mut blocks = [[[0u32; 16]; 16]; 16];
                                let mut blocks_data = blocks_data.as_slice();
                                for x in 0..16 {
                                    for y in 0..16 {
                                        for z in 0..16 {
                                            blocks[x][y][z] = blocks_data.read_be().unwrap();
                                        }
                                    }
                                }
                                world.load_chunk(ChunkPosition { x, y, z }, blocks);
                            }
                            NetworkMessageS2C::UnloadChunk(x, y, z) => {
                                world.unload_chunk(ChunkPosition { x, y, z });
                            }
                            NetworkMessageS2C::AddEntity(entity_type, id, x, y, z, rotation) => {
                                entities.insert(
                                    id,
                                    game::Entity {
                                        entity_type,
                                        rotation,
                                        position: Position { x, y, z },
                                        items: HashMap::new(),
                                    },
                                );
                            }
                            NetworkMessageS2C::MoveEntity(id, x, y, z, rotation) => {
                                if let Some(entity) = entities.get_mut(&id) {
                                    entity.position.x = x;
                                    entity.position.y = y;
                                    entity.position.z = z;
                                    entity.rotation = rotation;
                                }
                            }
                            NetworkMessageS2C::DeleteEntity(id) => {
                                entities.remove(&id);
                            }
                            NetworkMessageS2C::GuiData(data) => {
                                gui.on_json_data(data);
                            }
                            NetworkMessageS2C::BlockBreakTimeResponse(id, time) => {
                                block_breaking_manager.on_block_break_time_response(id, time);
                            }
                            NetworkMessageS2C::EntityAddItem(entity_id, item_index, item_id) => {
                                entities
                                    .get_mut(&entity_id)
                                    .unwrap()
                                    .items
                                    .insert(item_index, item_id);
                            }
                            NetworkMessageS2C::BlockAddItem(
                                x,
                                y,
                                z,
                                x_offset,
                                y_offset,
                                z_offset,
                                item_index,
                                item_id,
                            ) => {
                                let block_pos = BlockPosition { x, y, z };
                                if !world.blocks_with_items.contains_key(&block_pos) {
                                    world.blocks_with_items.insert(block_pos, HashMap::new());
                                }
                                world
                                    .blocks_with_items
                                    .get_mut(&block_pos)
                                    .unwrap()
                                    .insert(item_index, (x_offset, y_offset, z_offset, item_id));
                            }
                            NetworkMessageS2C::BlockRemoveItem(x, y, z, item_index) => {
                                if let Some(block_item_storage) =
                                    world.blocks_with_items.get_mut(&BlockPosition { x, y, z })
                                {
                                    block_item_storage.remove(&item_index);
                                }
                            }
                            NetworkMessageS2C::BlockMoveItem(
                                x,
                                y,
                                z,
                                x_offset,
                                y_offset,
                                z_offset,
                                item_index,
                            ) => {
                                if let Some(block_item_storage) =
                                    world.blocks_with_items.get_mut(&BlockPosition { x, y, z })
                                {
                                    if let Some(item) = block_item_storage.get_mut(&item_index) {
                                        item.0 = x_offset;
                                        item.1 = y_offset;
                                        item.2 = z_offset;
                                    }
                                }
                            }
                            NetworkMessageS2C::Knockback(x, y, z, set) => {
                                camera.knockback(x, y, z, set);
                            }
                            NetworkMessageS2C::FluidSelectable(selectable) => {
                                fluid_selectable = selectable;
                            }
                            NetworkMessageS2C::PlaySound(id, x, y, z, gain, pitch, relative) => {
                                sound_manager.play_sound(
                                    id,
                                    Position { x, y, z },
                                    gain,
                                    pitch,
                                    relative,
                                );
                            }
                        }
                    }
                    tungstenite::Message::Close(_) => {
                        panic!("connection closed");
                    }
                    _ => {}
                },
                Err(err) => match err {
                    tungstenite::Error::AlreadyClosed => panic!("connection closed"),
                    _ => {
                        break 'message_loop;
                    }
                },
            }
        }

        let raycast_result = raycast(
            &world,
            &camera,
            &block_registry,
            &entities,
            &entity_registry,
            fluid_selectable,
        );

        block_breaking_manager.set_target_block(match raycast_result {
            Some(ref raycast_result) => match &raycast_result {
                HitResult::Block(pos, _, face) => Some((pos.clone(), *face)),
                HitResult::Entity(_) => None,
            },
            None => None,
        });
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { timestamp: _ } => break 'main_loop,
                Event::MouseWheel {
                    timestamp: _,
                    window_id,
                    which: _,
                    x,
                    y,
                    direction: _,
                } => {
                    if window_id == win_id {
                        if !gui.on_mouse_scroll(
                            &mut socket,
                            x,
                            y,
                            keys_held.contains(&Keycode::LShift),
                        ) {
                            socket
                                .write_message(tungstenite::Message::Binary(
                                    util::NetworkMessageC2S::MouseScroll(x, y).to_data(),
                                ))
                                .unwrap();
                        }
                    }
                }
                Event::MouseMotion {
                    timestamp: _,
                    window_id,
                    which: _,
                    mousestate: _,
                    x,
                    y,
                    xrel,
                    yrel,
                } => {
                    if window_id == win_id {
                        if !gui.on_mouse_move(x, y) {
                            let sensitivity = 0.5f32;
                            camera.update_orientation(
                                (-yrel as f32) * sensitivity,
                                (-xrel as f32) * sensitivity,
                            );
                        }
                    }
                }
                Event::MouseButtonDown {
                    timestamp: _,
                    window_id,
                    which: _,
                    mouse_btn,
                    clicks: _,
                    x: _,
                    y: _,
                } => {
                    if window_id == win_id {
                        if mouse_btn == MouseButton::Middle {
                            break 'main_loop;
                        }
                        if mouse_btn == MouseButton::Left {
                            if !gui.on_left_click(&mut socket, keys_held.contains(&Keycode::LShift))
                            {
                                /*if let Some((position, _id, _face)) = raycast_result {
                                    socket
                                        .write_message(tungstenite::Message::Binary(
                                            NetworkMessageC2S::LeftClickBlock(
                                                position.x, position.y, position.z,
                                            )
                                            .to_data(),
                                        ))
                                        .unwrap();
                                }*/
                                if let Some(raycast_result) = &raycast_result {
                                    match raycast_result {
                                        HitResult::Block(_, _, _) => {}
                                        HitResult::Entity(id) => {
                                            socket
                                                .write_message(tungstenite::Message::Binary(
                                                    NetworkMessageC2S::LeftClickEntity(*id)
                                                        .to_data(),
                                                ))
                                                .unwrap();
                                        }
                                    }
                                }
                                block_breaking_manager.set_left_click_held(true);
                            }
                        }
                        if mouse_btn == MouseButton::Right {
                            if !gui.on_right_click() {
                                match &raycast_result {
                                    Some(HitResult::Block(position, _, face)) => {
                                        socket
                                            .write_message(tungstenite::Message::Binary(
                                                NetworkMessageC2S::RightClickBlock(
                                                    position.x,
                                                    position.y,
                                                    position.z,
                                                    *face,
                                                    camera.is_shifting(),
                                                )
                                                .to_data(),
                                            ))
                                            .unwrap();
                                    }
                                    Some(HitResult::Entity(id)) => {
                                        socket
                                            .write_message(tungstenite::Message::Binary(
                                                NetworkMessageC2S::RightClickEntity(*id).to_data(),
                                            ))
                                            .unwrap();
                                    }
                                    None => {
                                        socket
                                            .write_message(tungstenite::Message::Binary(
                                                NetworkMessageC2S::RightClick(camera.is_shifting())
                                                    .to_data(),
                                            ))
                                            .unwrap();
                                    }
                                }
                            }
                        }
                    }
                }
                Event::MouseButtonUp {
                    timestamp: _,
                    window_id,
                    which: _,
                    mouse_btn,
                    clicks: _,
                    x: _,
                    y: _,
                } => {
                    if window_id == win_id {
                        if mouse_btn == MouseButton::Left {
                            block_breaking_manager.set_left_click_held(false);
                        }
                    }
                }
                Event::KeyDown {
                    timestamp: _,
                    window_id,
                    keycode,
                    scancode: _,
                    keymod: _,
                    repeat,
                } => {
                    if window_id == win_id {
                        if keycode.unwrap() == Keycode::Escape {
                            socket
                                .write_message(tungstenite::Message::Binary(
                                    NetworkMessageC2S::GuiClose.to_data(),
                                ))
                                .unwrap();
                        }
                        keys_held.insert(keycode.unwrap());
                        socket
                            .write_message(tungstenite::Message::Binary(
                                NetworkMessageC2S::Keyboard(keycode.unwrap() as i32, true, repeat)
                                    .to_data(),
                            ))
                            .unwrap();
                    }
                }
                Event::KeyUp {
                    timestamp: _,
                    window_id,
                    keycode,
                    scancode: _,
                    keymod: _,
                    repeat,
                } => {
                    if window_id == win_id {
                        keys_held.remove(&keycode.unwrap());
                        socket
                            .write_message(tungstenite::Message::Binary(
                                NetworkMessageC2S::Keyboard(keycode.unwrap() as i32, false, repeat)
                                    .to_data(),
                            ))
                            .unwrap();
                    }
                }
                Event::Window {
                    timestamp: _,
                    window_id,
                    win_event,
                } => {
                    if window_id == win_id {
                        match win_event {
                            WindowEvent::Resized(width, height) => {
                                win_width = width as u32;
                                win_height = height as u32;
                                gui.size = (win_width, win_height);
                                unsafe {
                                    ogl33::glViewport(0, 0, win_width as i32, win_height as i32);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => (),
            }
        }
        unsafe {
            ogl33::glClear(ogl33::GL_COLOR_BUFFER_BIT | ogl33::GL_DEPTH_BUFFER_BIT);

            let time = timer.ticks() as f32 / 1000f32;
            let delta_time = time - last_time;
            last_time = time;

            fps += 1;
            if last_fps_time + 1000 < timer.ticks() {
                last_fps_cnt = fps;
                last_fps_time = timer.ticks();
                fps = 0;
            }

            block_breaking_manager.tick(delta_time, &mut socket, keys_held.contains(&Keycode::R));
            camera.update_position(&keys_held, delta_time, &world);
            sound_manager.tick(camera.position, camera.make_front());
            {
                window
                    .borrow_mut()
                    .set_title(
                        format!(
                            "BlockByte {:.1} {:.1} {:.1}",
                            camera.position.x, camera.position.y, camera.position.z
                        )
                        .as_str(),
                    )
                    .unwrap();
            }
            socket
                .write_message(tungstenite::Message::Binary(
                    NetworkMessageC2S::PlayerPosition(
                        camera.position.x - 0.3,
                        camera.position.y,
                        camera.position.z - 0.3,
                        camera.is_shifting(),
                        camera.yaw_deg,
                    )
                    .to_data(),
                ))
                .unwrap();

            chunk_shader.use_program();
            let projection_view_loc = chunk_shader
                .get_uniform_location("projection_view\0")
                .expect("transform uniform not found");
            chunk_shader.set_uniform_matrix(
                projection_view_loc,
                ultraviolet::projection::perspective_gl(
                    90f32.to_radians(),
                    (win_width as f32) / (win_height as f32),
                    0.01,
                    1000.,
                ) * camera.create_view_matrix(),
            );
            let rendered_chunks = {
                world.render(
                    &chunk_shader,
                    (timer.ticks() as f32) / 1000f32,
                    Position {
                        x: camera.position.x,
                        y: camera.position.y,
                        z: camera.position.z,
                    }
                    .to_chunk_pos(),
                )
            };

            model_shader.use_program();
            let projection_view_loc = model_shader
                .get_uniform_location("projection_view\0")
                .expect("transform uniform not found");
            let projection = ultraviolet::projection::perspective_gl(
                90f32.to_radians(),
                (win_width as f32) / (win_height as f32),
                0.01,
                1000.,
            ) * camera.create_view_matrix();
            model_shader.set_uniform_matrix(projection_view_loc, projection);
            let mut items_to_render_in_world = Vec::new();
            let mut models = Vec::new();
            for entity in &entities {
                let model = entity_registry.get(&entity.1.entity_type).unwrap();
                models.push((entity.1.position, "".to_string(), 0., &model.1));
                for item in &entity.1.items {
                    items_to_render_in_world.push((entity.1.position.clone(), *item.1));
                }
            }
            for block in &world.blocks_with_items {
                for item in block.1 {
                    items_to_render_in_world.push((
                        Position {
                            x: (block.0.x as f32) + item.1 .0,
                            y: (block.0.y as f32) + item.1 .1,
                            z: (block.0.z as f32) + item.1 .2,
                        },
                        item.1 .3,
                    ));
                }
            }
            world_entity_renderer.render(
                items_to_render_in_world,
                &item_registry,
                &projection,
                &texture_atlas,
                &block_registry,
                models,
            );
            particle_manager.tick(delta_time, &world, &block_registry);
            particle_manager.render(
                &ultraviolet::projection::perspective_gl(
                    90f32.to_radians(),
                    (win_width as f32) / (win_height as f32),
                    0.01,
                    1000.,
                ),
                &camera.create_view_matrix(),
            );
            outline_shader.use_program();
            let projection_view_loc = outline_shader
                .get_uniform_location("projection_view\0")
                .expect("transform uniform not found");
            outline_shader.set_uniform_matrix(projection_view_loc, projection.clone());
            if let Some(raycast_result) = &raycast_result {
                ogl33::glDisable(ogl33::GL_DEPTH_TEST);
                outline_renderer.render(
                    raycast_result,
                    &outline_shader,
                    &block_registry,
                    &entities,
                    &entity_registry,
                );
                ogl33::glEnable(ogl33::GL_DEPTH_TEST);
            }
            block_breaking_manager.render(&projection);
            sky_renderer.render(
                ultraviolet::projection::perspective_gl(
                    90f32.to_radians(),
                    (win_width as f32) / (win_height as f32),
                    0.01,
                    1000.,
                ) * camera.create_view_matrix_no_pos(),
                delta_time,
            );
            gui.render(&gui_shader, &camera.position, last_fps_cnt, rendered_chunks);
            {
                window.borrow().gl_swap_window();
            }
        }
    }
    discord_rpc_thread_tx.send(()).unwrap();
    discord_rpc_thread.join().unwrap();
}
struct SkyRenderer {
    vao: glwrappers::VertexArray,
    vbo: glwrappers::Buffer,
    shader: glwrappers::Shader,
    time: f32,
}
impl SkyRenderer {
    pub fn new() -> Self {
        let vao = glwrappers::VertexArray::new().unwrap();
        vao.bind();
        let mut vbo = glwrappers::Buffer::new(glwrappers::BufferType::Array).unwrap();
        vbo.bind();
        unsafe {
            ogl33::glVertexAttribPointer(
                0,
                3,
                ogl33::GL_FLOAT,
                ogl33::GL_FALSE,
                std::mem::size_of::<glwrappers::SkyVertex>()
                    .try_into()
                    .unwrap(),
                0 as *const _,
            );
            ogl33::glVertexAttribPointer(
                1,
                1,
                ogl33::GL_FLOAT,
                ogl33::GL_FALSE,
                std::mem::size_of::<glwrappers::SkyVertex>()
                    .try_into()
                    .unwrap(),
                std::mem::size_of::<[f32; 3]>() as *const _,
            );
            ogl33::glEnableVertexAttribArray(1);
            ogl33::glEnableVertexAttribArray(0);
        }
        let mut vertices = Vec::new();
        //up
        SkyRenderer::add_face(
            &mut vertices,
            (-1., 1., -1., 1.),
            (1., 1., -1., 1.),
            (1., 1., 1., 1.),
            (-1., 1., 1., 1.),
        );
        //down
        SkyRenderer::add_face(
            &mut vertices,
            (-1., -1., -1., 0.),
            (1., -1., -1., 0.),
            (1., -1., 1., 0.),
            (-1., -1., 1., 0.),
        );
        //north
        SkyRenderer::add_face(
            &mut vertices,
            (-1., -1., -1., 0.),
            (1., -1., -1., 0.),
            (1., 1., -1., 1.),
            (-1., 1., -1., 1.),
        );
        //south
        SkyRenderer::add_face(
            &mut vertices,
            (-1., -1., 1., 0.),
            (1., -1., 1., 0.),
            (1., 1., 1., 1.),
            (-1., 1., 1., 1.),
        );
        //left
        SkyRenderer::add_face(
            &mut vertices,
            (-1., -1., -1., 0.),
            (-1., -1., 1., 0.),
            (-1., 1., 1., 1.),
            (-1., 1., -1., 1.),
        );
        //right
        SkyRenderer::add_face(
            &mut vertices,
            (1., -1., -1., 0.),
            (1., -1., 1., 0.),
            (1., 1., 1., 1.),
            (1., 1., -1., 1.),
        );
        vbo.upload_data(
            bytemuck::cast_slice(vertices.as_slice()),
            ogl33::GL_STATIC_DRAW,
        );
        SkyRenderer {
            shader: glwrappers::Shader::new(
                include_str!("shaders/sky.vert").to_string(),
                include_str!("shaders/sky.frag").to_string(),
            ),
            time: 0.,
            vbo,
            vao,
        }
    }
    fn add_face(
        vertices: &mut Vec<glwrappers::SkyVertex>,
        p1: (f32, f32, f32, f32),
        p2: (f32, f32, f32, f32),
        p3: (f32, f32, f32, f32),
        p4: (f32, f32, f32, f32),
    ) {
        vertices.push([p1.0, p1.1, p1.2, p1.3]);
        vertices.push([p2.0, p2.1, p2.2, p2.3]);
        vertices.push([p3.0, p3.1, p3.2, p3.3]);
        vertices.push([p3.0, p3.1, p3.2, p3.3]);
        vertices.push([p4.0, p4.1, p4.2, p4.3]);
        vertices.push([p1.0, p1.1, p1.2, p1.3]);
    }
    pub fn render(&mut self, view: Mat4, delta_time: f32) {
        self.shader.use_program();
        self.vao.bind();
        let projection_view_loc = self
            .shader
            .get_uniform_location("projection_view\0")
            .expect("transform uniform not found");
        let time_loc = self.shader.get_uniform_location("daytime\0").unwrap();
        self.shader.set_uniform_matrix(projection_view_loc, view);
        self.shader.set_uniform_float(time_loc, self.time);
        //self.time += delta_time * 0.05;
        if self.time > 2. {
            self.time = 0.;
        }
        unsafe {
            ogl33::glDepthFunc(ogl33::GL_LEQUAL);
            ogl33::glDrawArrays(ogl33::GL_TRIANGLES, 0, 6 * 2 * 3);
            ogl33::glDepthFunc(ogl33::GL_LESS);
        }
    }
}
fn pack_textures(
    textures: Vec<(String, std::path::PathBuf)>,
    font: &rusttype::Font,
) -> (TextureAtlas, RgbaImage) {
    let mut texture_map = std::collections::HashMap::new();
    let mut packer =
        texture_packer::TexturePacker::new_skyline(texture_packer::TexturePackerConfig {
            max_width: 2048,
            max_height: 2048,
            allow_rotation: false,
            texture_outlines: false,
            border_padding: 0,
            texture_padding: 0,
            trim: false,
            texture_extrusion: 0,
        });
    for (name, path) in textures {
        if let Ok(texture) = ImageImporter::import_from_file(path.as_path()) {
            packer.pack_own(name, texture).unwrap();
        }
    }
    {
        let glyphs: Vec<_> = (0..font.glyph_count())
            .map(|i| {
                font.glyph(GlyphId { 0: i as u16 })
                    .scaled(Scale::uniform(30.))
                    .positioned(Point { x: 0., y: 0. })
            })
            .collect();
        for g in glyphs.iter().enumerate() {
            if let Some(bb) = g.1.pixel_bounding_box() {
                let mut font_texture =
                    DynamicImage::new_rgba8(bb.width() as u32, bb.height() as u32);
                let font_buffer = match &mut font_texture {
                    DynamicImage::ImageRgba8(buffer) => buffer,
                    _ => panic!(),
                };
                g.1.draw(|x, y, v| {
                    font_buffer.put_pixel(x, y, Rgba([0, 0, 0, (v * 255f32) as u8]));
                    //font_buffer.put_pixel(x, y, Rgba([(v * 255f32) as u8, 0, 0, 255]));
                });
                packer
                    .pack_own("font_".to_string() + g.0.to_string().as_str(), font_texture)
                    .unwrap();
            }
        }
    }
    packer
        .pack_own(
            "missing".to_string(),
            ImageImporter::import_from_memory(include_bytes!("missing.png"))
                .expect("missing texture corrupted"),
        )
        .unwrap();
    for (name, frame) in packer.get_frames() {
        let texture = game::AtlassedTexture {
            x: frame.frame.x,
            y: frame.frame.y,
            w: frame.frame.w,
            h: frame.frame.h,
            atlas_w: packer.width(),
            atlas_h: packer.height(),
        };
        texture_map.insert(name.to_string(), texture);
    }
    let exporter = ImageExporter::export(&packer).unwrap();
    exporter.save(Path::new("textureatlasdump.png")).unwrap();
    (
        TextureAtlas {
            missing_texture: texture_map.get("missing").unwrap().clone(),
            textures: texture_map,
        },
        exporter.to_rgba8(),
    )
}
struct WorldEntityRenderer {
    vao: glwrappers::VertexArray,
    vbo: glwrappers::Buffer,
    shader: glwrappers::Shader,
}
impl WorldEntityRenderer {
    pub fn new() -> Self {
        let vao = glwrappers::VertexArray::new().expect("couldnt create vao for outline renderer");
        vao.bind();
        let vbo = glwrappers::Buffer::new(glwrappers::BufferType::Array)
            .expect("couldnt create vbo for chunk");
        vbo.bind();
        unsafe {
            ogl33::glVertexAttribPointer(
                0,
                3,
                ogl33::GL_FLOAT,
                ogl33::GL_FALSE,
                std::mem::size_of::<glwrappers::BasicVertex>()
                    .try_into()
                    .unwrap(),
                0 as *const _,
            );
            ogl33::glVertexAttribPointer(
                1,
                2,
                ogl33::GL_FLOAT,
                ogl33::GL_FALSE,
                std::mem::size_of::<glwrappers::BasicVertex>()
                    .try_into()
                    .unwrap(),
                std::mem::size_of::<[f32; 3]>() as *const _,
            );
            ogl33::glEnableVertexAttribArray(1);
            ogl33::glEnableVertexAttribArray(0);
        }
        WorldEntityRenderer {
            vao,
            vbo,
            shader: glwrappers::Shader::new(
                include_str!("shaders/basic.vert").to_string(),
                include_str!("shaders/basic.frag").to_string(),
            ),
        }
    }
    fn add_face(
        vertices: &mut Vec<glwrappers::BasicVertex>,
        x1: f32,
        y1: f32,
        z1: f32,
        x2: f32,
        y2: f32,
        z2: f32,
        x3: f32,
        y3: f32,
        z3: f32,
        x4: f32,
        y4: f32,
        z4: f32,
        uv: (f32, f32, f32, f32),
    ) {
        vertices.push([x1, y1, z1, uv.0, uv.1]);
        vertices.push([x2, y2, z2, uv.2, uv.1]);
        vertices.push([x3, y3, z3, uv.2, uv.3]);
        vertices.push([x3, y3, z3, uv.2, uv.3]);
        vertices.push([x4, y4, z4, uv.0, uv.3]);
        vertices.push([x1, y1, z1, uv.0, uv.1]);
    }
    pub fn render(
        &mut self,
        items: Vec<(Position, u32)>,
        item_registry: &HashMap<u32, ItemRenderData>,
        projection: &Mat4,
        texture_atlas: &TextureAtlas,
        block_registry: &BlockRegistry,
        models: Vec<(Position, String, f32, &model::Model)>,
    ) {
        let mut vertices: Vec<glwrappers::BasicVertex> = Vec::new();
        let mut vertex_count = 0;
        for item in &items {
            let item_texture = &item_registry.get(&item.1).unwrap().model;
            let position = &item.0;
            match item_texture {
                ItemModel::Texture(texture) => {
                    let uv = texture_atlas.get(texture.as_str()).get_coords();
                    WorldEntityRenderer::add_face(
                        &mut vertices,
                        position.x,
                        position.y,
                        position.z,
                        position.x + 0.5,
                        position.y,
                        position.z,
                        position.x + 0.5,
                        position.y,
                        position.z + 0.5,
                        position.x,
                        position.y,
                        position.z + 0.5,
                        uv,
                    );
                    vertex_count += 6;
                }
                ItemModel::Block(block) => {
                    let block = block_registry.get_block(*block);
                    match block.render_type {
                        BlockRenderType::Air => {}
                        BlockRenderType::Cube(_, north, south, right, left, up, down) => {
                            WorldEntityRenderer::add_face(
                                &mut vertices,
                                position.x,
                                position.y,
                                position.z,
                                position.x + 0.5,
                                position.y,
                                position.z,
                                position.x + 0.5,
                                position.y,
                                position.z + 0.5,
                                position.x,
                                position.y,
                                position.z + 0.5,
                                down.get_coords(),
                            );
                            WorldEntityRenderer::add_face(
                                &mut vertices,
                                position.x,
                                position.y + 0.5,
                                position.z,
                                position.x + 0.5,
                                position.y + 0.5,
                                position.z,
                                position.x + 0.5,
                                position.y + 0.5,
                                position.z + 0.5,
                                position.x,
                                position.y + 0.5,
                                position.z + 0.5,
                                up.get_coords(),
                            );
                            WorldEntityRenderer::add_face(
                                &mut vertices,
                                position.x,
                                position.y + 0.5,
                                position.z,
                                position.x + 0.5,
                                position.y + 0.5,
                                position.z,
                                position.x + 0.5,
                                position.y,
                                position.z,
                                position.x,
                                position.y,
                                position.z,
                                north.get_coords(),
                            );
                            WorldEntityRenderer::add_face(
                                &mut vertices,
                                position.x,
                                position.y + 0.5,
                                position.z + 0.5,
                                position.x + 0.5,
                                position.y + 0.5,
                                position.z + 0.5,
                                position.x + 0.5,
                                position.y,
                                position.z + 0.5,
                                position.x,
                                position.y,
                                position.z + 0.5,
                                south.get_coords(),
                            );
                            WorldEntityRenderer::add_face(
                                &mut vertices,
                                position.x,
                                position.y + 0.5,
                                position.z,
                                position.x,
                                position.y + 0.5,
                                position.z + 0.5,
                                position.x,
                                position.y,
                                position.z + 0.5,
                                position.x,
                                position.y,
                                position.z,
                                left.get_coords(),
                            );
                            WorldEntityRenderer::add_face(
                                &mut vertices,
                                position.x + 0.5,
                                position.y + 0.5,
                                position.z,
                                position.x + 0.5,
                                position.y + 0.5,
                                position.z + 0.5,
                                position.x + 0.5,
                                position.y,
                                position.z + 0.5,
                                position.x + 0.5,
                                position.y,
                                position.z,
                                right.get_coords(),
                            );
                            vertex_count += 6 * 6;
                        }
                        BlockRenderType::StaticModel(_, _, _, _, _, _, _, _, _, _) => {}
                        BlockRenderType::Foliage(_, _, _, _) => {}
                    }
                }
            }
        }
        for model in models {
            model.3.add_vertices(
                &mut |pos, u, v| {
                    vertices.push([pos.x, pos.y, pos.z, u, v]);
                    vertex_count += 1;
                },
                Some((model.1, model.2)),
                Vec3::new(model.0.x, model.0.y, model.0.z),
                Vec3::new(0., (model.2 * 100.).to_radians() * 0., 0.),
                Vec3::new(-0.125, 0., -0.125),
                Vec3::new(1., 1., 1.),
            );
        }
        self.vao.bind();
        self.vbo.upload_data(
            bytemuck::cast_slice(vertices.as_slice()),
            ogl33::GL_STREAM_DRAW,
        );
        self.shader.use_program();
        self.shader.set_uniform_matrix(
            self.shader
                .get_uniform_location("projection_view\0")
                .unwrap(),
            projection.clone(),
        );
        unsafe {
            ogl33::glBlendFunc(ogl33::GL_SRC_ALPHA, ogl33::GL_ONE_MINUS_SRC_ALPHA);
            ogl33::glEnable(ogl33::GL_BLEND);
            ogl33::glDrawArrays(ogl33::GL_TRIANGLES, 0, vertex_count);
            ogl33::glDisable(ogl33::GL_BLEND);
        }
    }
}
#[derive(Clone)]
pub struct TextureAtlas {
    textures: HashMap<String, AtlassedTexture>,
    missing_texture: AtlassedTexture,
}
impl TextureAtlas {
    pub fn get(&self, texture: &str) -> &AtlassedTexture {
        self.textures.get(texture).unwrap_or(&self.missing_texture)
    }
}
pub enum HitResult {
    Block(BlockPosition, u32, Face),
    Entity(u32),
}
pub fn raycast(
    world: &game::World,
    camera: &game::ClientPlayer,
    block_registry: &BlockRegistry,
    entities: &HashMap<u32, game::Entity>,
    entity_registry: &HashMap<u32, (EntityRenderData, model::Model)>,
    fluid_selectable: bool,
) -> Option<HitResult> {
    //TODO: better algorithm
    let mut ray_pos = camera.get_eye().clone();
    let dir = camera.make_front().normalized() * 0.03;
    let mut last_pos = ray_pos.clone();
    for _ in 0..200 {
        let position = BlockPosition {
            x: ray_pos.x.floor() as i32,
            y: ray_pos.y.floor() as i32,
            z: ray_pos.z.floor() as i32,
        };
        for entry in entities {
            let entity = entry.1;
            let entity_render_data = &entity_registry.get(&entity.entity_type).unwrap().0;
            if ray_pos.x >= entity.position.x
                && ray_pos.x <= (entity.position.x + entity_render_data.hitbox_w)
                && ray_pos.y >= entity.position.y
                && ray_pos.y <= (entity.position.y + entity_render_data.hitbox_h)
                && ray_pos.z >= entity.position.z
                && ray_pos.z <= (entity.position.z + entity_render_data.hitbox_d)
            {
                return Some(HitResult::Entity(*entry.0));
            }
        }
        if let Some(id) = world.get_block(position) {
            let block = block_registry.get_block(id);
            if !(block.fluid && !fluid_selectable) {
                match &block.render_type {
                    BlockRenderType::Air => {}
                    BlockRenderType::Cube(_, _, _, _, _, _, _)
                    | BlockRenderType::Foliage(_, _, _, _)
                    | BlockRenderType::StaticModel(_, _, _, _, _, _, _, _, _, _) => {
                        let last_pos = last_pos.to_block_pos();
                        let mut least_diff_face = Face::Up;
                        for face in Face::all() {
                            let offset = face.get_offset();
                            let diff = (position.x - last_pos.x + offset.x).abs()
                                + (position.y - last_pos.y + offset.y).abs()
                                + (position.z - last_pos.z + offset.z).abs();
                            if diff <= 1 {
                                least_diff_face = face.clone();
                            }
                        }
                        return Some(HitResult::Block(position, id, least_diff_face));
                    } /* => {
                          let x = ray_pos.x - (position.x as f32);
                          let y = ray_pos.y - (position.y as f32);
                          let z = ray_pos.z - (position.z as f32);
                          for cube in &model.cubes {
                              if x >= cube.from.x
                                  && x <= cube.to.x
                                  && y >= cube.from.y
                                  && y <= cube.to.y
                                  && z >= cube.from.z
                                  && z <= cube.to.z
                              {
                                  let size = Position {
                                      x: cube.to.x - cube.from.x,
                                      y: cube.to.y - cube.from.y,
                                      z: cube.to.z - cube.from.z,
                                  };
                                  let mut least_diff_face = Face::Up;
                                  let mut least_diff = 10.;
                                  for face in Face::all() {
                                      let offset = face.get_offset();
                                      let diff = (((x - cube.from.x) / size.x) - 0.5
                                          + (offset.x as f32))
                                          .abs()
                                          + (((y - cube.from.y) / size.y) - 0.5 + (offset.y as f32))
                                              .abs()
                                          + (((z - cube.from.z) / size.z) - 0.5 + (offset.z as f32))
                                              .abs();
                                      if diff <= least_diff {
                                          least_diff = diff;
                                          least_diff_face = face.clone();
                                      }
                                  }
                                  return Some(HitResult::Block(
                                      position,
                                      id,
                                      least_diff_face.opposite(),
                                  ));
                              }
                          }
                      }*/
                }
            }
        }
        last_pos = ray_pos.clone();
        ray_pos = ray_pos.add(dir.x, dir.y, dir.z);
    }
    None
}

struct BlockOutline {
    vao: glwrappers::VertexArray,
    vbo: glwrappers::Buffer,
    last_id: u32,
    last_entity: bool,
    vertex_count: i32,
}
impl BlockOutline {
    pub fn new() -> Self {
        let vao = glwrappers::VertexArray::new().expect("couldnt create vao for outline renderer");
        vao.bind();
        let vbo = glwrappers::Buffer::new(glwrappers::BufferType::Array)
            .expect("couldnt create vbo for chunk");
        vbo.bind();
        unsafe {
            ogl33::glVertexAttribPointer(
                0,
                3,
                ogl33::GL_FLOAT,
                ogl33::GL_FALSE,
                std::mem::size_of::<glwrappers::ColorVertex>()
                    .try_into()
                    .unwrap(),
                0 as *const _,
            );
            ogl33::glVertexAttribPointer(
                1,
                3,
                ogl33::GL_FLOAT,
                ogl33::GL_FALSE,
                std::mem::size_of::<glwrappers::ColorVertex>()
                    .try_into()
                    .unwrap(),
                std::mem::size_of::<[f32; 3]>() as *const _,
            );
            ogl33::glEnableVertexAttribArray(1);
            ogl33::glEnableVertexAttribArray(0);
        }
        BlockOutline {
            vao,
            vbo,
            last_id: 0,
            last_entity: true,
            vertex_count: 0,
        }
    }
    fn upload_cube(&mut self, r: f32, g: f32, b: f32) {
        let mut vertices: Vec<glwrappers::ColorVertex> = Vec::new();
        vertices.push([0., 0., 0., r, g, b]);
        vertices.push([1., 0., 0., r, g, b]);
        vertices.push([1., 0., 0., r, g, b]);
        vertices.push([1., 0., 1., r, g, b]);
        vertices.push([1., 0., 1., r, g, b]);
        vertices.push([0., 0., 1., r, g, b]);
        vertices.push([0., 0., 1., r, g, b]);
        vertices.push([0., 0., 0., r, g, b]);

        vertices.push([0., 1., 0., r, g, b]);
        vertices.push([1., 1., 0., r, g, b]);
        vertices.push([1., 1., 0., r, g, b]);
        vertices.push([1., 1., 1., r, g, b]);
        vertices.push([1., 1., 1., r, g, b]);
        vertices.push([0., 1., 1., r, g, b]);
        vertices.push([0., 1., 1., r, g, b]);
        vertices.push([0., 1., 0., r, g, b]);

        vertices.push([0., 0., 0., r, g, b]);
        vertices.push([0., 1., 0., r, g, b]);
        vertices.push([1., 0., 0., r, g, b]);
        vertices.push([1., 1., 0., r, g, b]);
        vertices.push([1., 0., 1., r, g, b]);
        vertices.push([1., 1., 1., r, g, b]);
        vertices.push([0., 0., 1., r, g, b]);
        vertices.push([0., 1., 1., r, g, b]);
        self.vbo.upload_data(
            bytemuck::cast_slice(vertices.as_slice()),
            ogl33::GL_STATIC_DRAW,
        );
        self.vertex_count = 24;
    }
    /*fn upload_static_model(&mut self, model: &StaticBlockModel, r: f32, g: f32, b: f32) {
        let mut vertices: Vec<glwrappers::ColorVertex> = Vec::new();
        for cube in &model.cubes {
            let from = cube.from;
            let to = cube.to;
            vertices.push([from.x, from.y, from.z, r, g, b]);
            vertices.push([to.x, from.y, from.z, r, g, b]);
            vertices.push([to.x, from.y, from.z, r, g, b]);
            vertices.push([to.x, from.y, to.z, r, g, b]);
            vertices.push([to.x, from.y, to.z, r, g, b]);
            vertices.push([from.x, from.y, to.z, r, g, b]);
            vertices.push([from.x, from.y, to.z, r, g, b]);
            vertices.push([from.x, from.y, from.z, r, g, b]);

            vertices.push([from.x, to.y, from.z, r, g, b]);
            vertices.push([to.x, to.y, from.z, r, g, b]);
            vertices.push([to.x, to.y, from.z, r, g, b]);
            vertices.push([to.x, to.y, to.z, r, g, b]);
            vertices.push([to.x, to.y, to.z, r, g, b]);
            vertices.push([from.x, to.y, to.z, r, g, b]);
            vertices.push([from.x, to.y, to.z, r, g, b]);
            vertices.push([from.x, to.y, from.z, r, g, b]);

            vertices.push([from.x, from.y, from.z, r, g, b]);
            vertices.push([from.x, to.y, from.z, r, g, b]);
            vertices.push([to.x, from.y, from.z, r, g, b]);
            vertices.push([to.x, to.y, from.z, r, g, b]);
            vertices.push([to.x, from.y, to.z, r, g, b]);
            vertices.push([to.x, to.y, to.z, r, g, b]);
            vertices.push([from.x, from.y, to.z, r, g, b]);
            vertices.push([from.x, to.y, to.z, r, g, b]);
        }
        self.vbo.upload_data(
            bytemuck::cast_slice(vertices.as_slice()),
            ogl33::GL_STATIC_DRAW,
        );
        self.vertex_count = 24 * model.cubes.len() as i32;
    }*/
    pub fn upload_entity(&mut self, entity_render_data: &EntityRenderData, r: f32, g: f32, b: f32) {
        let mut vertices: Vec<glwrappers::ColorVertex> = Vec::new();
        let w = entity_render_data.hitbox_w;
        let h = entity_render_data.hitbox_h;
        let d = entity_render_data.hitbox_d;
        vertices.push([0., 0., 0., r, g, b]);
        vertices.push([w, 0., 0., r, g, b]);
        vertices.push([w, 0., 0., r, g, b]);
        vertices.push([w, 0., d, r, g, b]);
        vertices.push([w, 0., d, r, g, b]);
        vertices.push([0., 0., d, r, g, b]);
        vertices.push([0., 0., d, r, g, b]);
        vertices.push([0., 0., 0., r, g, b]);

        vertices.push([0., h, 0., r, g, b]);
        vertices.push([w, h, 0., r, g, b]);
        vertices.push([w, h, 0., r, g, b]);
        vertices.push([w, h, d, r, g, b]);
        vertices.push([w, h, d, r, g, b]);
        vertices.push([0., h, d, r, g, b]);
        vertices.push([0., h, d, r, g, b]);
        vertices.push([0., h, 0., r, g, b]);

        vertices.push([0., 0., 0., r, g, b]);
        vertices.push([0., h, 0., r, g, b]);
        vertices.push([w, 0., 0., r, g, b]);
        vertices.push([w, h, 0., r, g, b]);
        vertices.push([w, 0., d, r, g, b]);
        vertices.push([w, h, d, r, g, b]);
        vertices.push([0., 0., d, r, g, b]);
        vertices.push([0., h, d, r, g, b]);
        self.vbo.upload_data(
            bytemuck::cast_slice(vertices.as_slice()),
            ogl33::GL_STATIC_DRAW,
        );
        self.vertex_count = 24;
    }
    pub fn render(
        &mut self,
        hit_result: &HitResult,
        shader: &glwrappers::Shader,
        block_registry: &BlockRegistry,
        entities: &HashMap<u32, game::Entity>,
        entity_registry: &HashMap<u32, (EntityRenderData, model::Model)>,
    ) {
        self.vao.bind();
        self.vbo.bind();
        let position = match hit_result {
            HitResult::Block(position, _, _) => Position {
                x: position.x as f32,
                y: position.y as f32,
                z: position.z as f32,
            },
            HitResult::Entity(id) => entities.get(id).unwrap().position.clone(),
        };
        shader.set_uniform_matrix(
            shader
                .get_uniform_location("model\0")
                .expect("uniform model not found"),
            ultraviolet::Mat4::from_translation(ultraviolet::Vec3 {
                x: (position.x) as f32,
                y: (position.y) as f32,
                z: (position.z) as f32,
            }),
        );
        match hit_result {
            HitResult::Block(_, id, _) => {
                if self.last_entity {
                    self.upload_cube(1., 0., 0.);
                    self.last_entity = false;
                }
            }
            HitResult::Entity(id) => {
                if (!self.last_entity) || self.last_id != *id {
                    self.upload_entity(
                        &entity_registry
                            .get(&entities.get(id).unwrap().entity_type)
                            .unwrap()
                            .0,
                        1.,
                        0.,
                        0.,
                    );
                    self.last_id = *id;
                    self.last_entity = true;
                }
            }
        }
        unsafe {
            ogl33::glDrawArrays(ogl33::GL_LINES, 0, self.vertex_count);
        }
    }
}
struct BlockBreakingManager {
    id: u32,
    time_requested: bool,
    target_block: Option<(BlockPosition, Face)>,
    key_down: bool,
    breaking_animation: Option<(f32, f32)>,
    block_breaking_shader: glwrappers::Shader,
    vao: glwrappers::VertexArray,
    vbo: glwrappers::Buffer,
    breaking_textures: Vec<AtlassedTexture>,
    just_pressed: bool,
}
impl BlockBreakingManager {
    pub fn new(breaking_textures: Vec<AtlassedTexture>) -> Self {
        let vao = VertexArray::new().unwrap();
        vao.bind();
        let vbo = Buffer::new(glwrappers::BufferType::Array).unwrap();
        vbo.bind();
        unsafe {
            ogl33::glVertexAttribPointer(
                0,
                3,
                ogl33::GL_FLOAT,
                ogl33::GL_FALSE,
                std::mem::size_of::<glwrappers::BasicVertex>()
                    .try_into()
                    .unwrap(),
                0 as *const _,
            );
            ogl33::glVertexAttribPointer(
                1,
                2,
                ogl33::GL_FLOAT,
                ogl33::GL_FALSE,
                std::mem::size_of::<glwrappers::BasicVertex>()
                    .try_into()
                    .unwrap(),
                std::mem::size_of::<[f32; 3]>() as *const _,
            );
            ogl33::glEnableVertexAttribArray(1);
            ogl33::glEnableVertexAttribArray(0);
        }
        BlockBreakingManager {
            id: 0,
            target_block: None,
            breaking_animation: None,
            key_down: false,
            time_requested: false,
            block_breaking_shader: glwrappers::Shader::new(
                include_str!("shaders/basic.vert").to_string(),
                include_str!("shaders/basic.frag").to_string(),
            ),
            vao,
            vbo,
            breaking_textures,
            just_pressed: false,
        }
    }
    pub fn tick(
        &mut self,
        delta_time: f32,
        socket: &mut WebSocket<TcpStream>,
        keep_breaking: bool,
    ) {
        if let Some(target_block) = self.target_block {
            if self.key_down
                && self.breaking_animation.is_none()
                && !self.time_requested
                && (keep_breaking || self.just_pressed)
            {
                self.time_requested = true;
                self.id += 1;
                socket
                    .write_message(tungstenite::Message::Binary(
                        NetworkMessageC2S::RequestBlockBreakTime(self.id, target_block.0).to_data(),
                    ))
                    .unwrap();
            }
        }
        if let Some(breaking_animation) = &mut self.breaking_animation {
            if let Some(target_block) = self.target_block {
                breaking_animation.0 += delta_time;
                if breaking_animation.0 >= breaking_animation.1 {
                    self.breaking_animation = None;
                    socket
                        .write_message(tungstenite::Message::Binary(
                            NetworkMessageC2S::BreakBlock(
                                target_block.0.x,
                                target_block.0.y,
                                target_block.0.z,
                            )
                            .to_data(),
                        ))
                        .unwrap();
                }
            }
        }
        self.just_pressed = false;
    }
    pub fn render(&mut self, projection: &Mat4) {
        if let Some(target_block) = self.target_block {
            if let Some(breaking_animation) = self.breaking_animation {
                let breaking_progress = breaking_animation.0 / breaking_animation.1;
                let breaking_texture = self
                    .breaking_textures
                    .get((breaking_progress * self.breaking_textures.len() as f32) as usize)
                    .unwrap();
                let uv = breaking_texture.get_coords();
                self.vao.bind();
                self.vbo.bind();
                let mut face_vertices = target_block.1.get_vertices();
                for vertex in &mut face_vertices {
                    vertex.x += target_block.0.x as f32;
                    vertex.y += target_block.0.y as f32;
                    vertex.z += target_block.0.z as f32;
                }
                let mut vertices: Vec<glwrappers::BasicVertex> = Vec::new();
                vertices.push([
                    face_vertices[0].x,
                    face_vertices[0].y,
                    face_vertices[0].z,
                    uv.0,
                    uv.1,
                ]);
                vertices.push([
                    face_vertices[1].x,
                    face_vertices[1].y,
                    face_vertices[1].z,
                    uv.2,
                    uv.1,
                ]);
                vertices.push([
                    face_vertices[2].x,
                    face_vertices[2].y,
                    face_vertices[2].z,
                    uv.2,
                    uv.3,
                ]);
                vertices.push([
                    face_vertices[2].x,
                    face_vertices[2].y,
                    face_vertices[2].z,
                    uv.2,
                    uv.3,
                ]);
                vertices.push([
                    face_vertices[3].x,
                    face_vertices[3].y,
                    face_vertices[3].z,
                    uv.0,
                    uv.3,
                ]);
                vertices.push([
                    face_vertices[0].x,
                    face_vertices[0].y,
                    face_vertices[0].z,
                    uv.0,
                    uv.1,
                ]);
                self.vbo.upload_data(
                    bytemuck::cast_slice(vertices.as_slice()),
                    ogl33::GL_DYNAMIC_DRAW,
                );
                self.block_breaking_shader.use_program();
                self.block_breaking_shader.set_uniform_matrix(
                    self.block_breaking_shader
                        .get_uniform_location("projection_view\0")
                        .unwrap(),
                    projection.clone(),
                );
                unsafe {
                    ogl33::glBlendFunc(ogl33::GL_SRC_ALPHA, ogl33::GL_ONE_MINUS_SRC_ALPHA);
                    ogl33::glEnable(ogl33::GL_BLEND);
                    ogl33::glDisable(ogl33::GL_DEPTH_TEST);
                    ogl33::glDrawArrays(ogl33::GL_TRIANGLES, 0, 6);
                    ogl33::glEnable(ogl33::GL_DEPTH_TEST);
                    ogl33::glDisable(ogl33::GL_BLEND);
                }
            }
        }
    }
    pub fn on_block_break_time_response(&mut self, id: u32, time: f32) {
        if self.id == id {
            self.breaking_animation = Some((0., time));
            self.time_requested = false;
        }
    }
    pub fn set_left_click_held(&mut self, held: bool) {
        if (!self.key_down) && held {
            self.just_pressed = true;
        }
        self.time_requested = false;
        self.key_down = held;
        if !held {
            self.breaking_animation = None;
        }
    }
    pub fn set_target_block(&mut self, block: Option<(BlockPosition, Face)>) {
        if match (self.target_block, block) {
            (Some(previous), Some(current)) => previous.0 != current.0,
            _ => true,
        } {
            self.breaking_animation = None;
            self.time_requested = false;
        }
        self.target_block = block;
    }
}
