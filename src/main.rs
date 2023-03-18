#![allow(dead_code)]
#![feature(hash_drain_filter)]

mod game;
mod glwrappers;
mod gui;
mod util;

use std::cell::RefCell;
use std::collections::HashMap;
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

use game::AtlassedTexture;
use game::Block;
use game::BlockRegistry;
use game::BlockRenderType;
use game::StaticBlockModel;
use glwrappers::Buffer;
use glwrappers::VertexArray;
use image::EncodableLayout;
use image::RgbaImage;
use ogl33::c_char;
use ogl33::c_void;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseButton;
use sdl2::video::SwapInterval;
use texture_packer::{exporter::ImageExporter, importer::ImageImporter, texture::Texture};
use tungstenite::WebSocket;
use ultraviolet::Mat4;
use util::*;

use endio::BERead;

use std::rc::Rc;

use sdl2::event::*;

fn main() {
    let addr = std::env::args().nth(1).unwrap();
    let tcp_stream = std::net::TcpStream::connect(addr).unwrap();
    let (mut socket, _response) = tungstenite::client::client_with_config(
        url::Url::parse("ws://aaa123").unwrap(),
        tcp_stream,
        None,
    )
    .unwrap();
    socket.get_mut().set_nonblocking(true).unwrap();
    let sdl = sdl2::init().unwrap();
    let video_subsystem = sdl.video().unwrap();
    let window = RefCell::new(
        video_subsystem
            .window("Game", 900, 700)
            .opengl()
            //.fullscreen_desktop()
            .resizable()
            .build()
            .unwrap(),
    );
    let _gl_context = { window.borrow().gl_create_context().unwrap() }; //do not drop

    let mut camera = game::ClientPlayer::at_position(ultraviolet::Vec3 {
        x: 0f32,
        y: 50f32,
        z: 0f32,
    });
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
        ogl33::glViewport(0, 0, win_width as i32, win_height as i32);
    }
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
    {}
    let (texture_atlas, packed_texture) = pack_textures(vec![
        ("dirt", std::path::Path::new("dirt.png")),
        ("grass", std::path::Path::new("grass.png")),
        ("grass_side", std::path::Path::new("grass_side.png")),
        ("cobble", std::path::Path::new("cobble.png")),
        ("player", std::path::Path::new("player.png")),
        ("font", std::path::Path::new("font.png")),
        ("slot", std::path::Path::new("slot.png")),
        ("cursor", std::path::Path::new("cursor.png")),
        ("arrow", std::path::Path::new("arrow.png")),
        ("breaking1", std::path::Path::new("breaking1.png")),
        ("breaking2", std::path::Path::new("breaking2.png")),
        ("breaking3", std::path::Path::new("breaking3.png")),
        ("breaking4", std::path::Path::new("breaking4.png")),
        ("breaking5", std::path::Path::new("breaking5.png")),
        ("breaking6", std::path::Path::new("breaking6.png")),
        ("breaking7", std::path::Path::new("breaking7.png")),
        ("breaking8", std::path::Path::new("breaking8.png")),
        ("breaking9", std::path::Path::new("breaking9.png")),
    ]);
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
    let player_texture = texture_atlas.get("player");
    let model = game::EntityModel::new(
        json::parse(
            std::fs::read_to_string(std::path::Path::new("player.bbmodel"))
                .unwrap()
                .as_str(),
        )
        .unwrap(),
        &player_texture,
    );
    let block_registry: Arc<Mutex<game::BlockRegistry>> =
        Arc::new(Mutex::new(game::BlockRegistry {
            blocks: vec![game::Block::new_air()],
        }));
    let item_registry = Rc::new(RefCell::new(Vec::new()));
    let mut outline_renderer = BlockOutline::new();
    let mut world = game::World::new(&block_registry);
    let mut event_pump = sdl.event_pump().unwrap();
    let timer = sdl.timer().unwrap();
    let mut gui = gui::GUI::new(
        gui::TextRenderer {
            texture: texture_atlas.get("font").clone(),
        },
        item_registry.clone(),
        texture_atlas.clone(),
        &sdl,
        (win_width, win_height),
        &window,
        block_registry.clone(),
    );
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
    let mut last_frame_time = 0f32;
    let (chunk_builder_input_tx, chunk_builder_input_rx) = std::sync::mpsc::channel();
    let (chunk_builder_output_tx, chunk_builder_output_rx) = std::sync::mpsc::channel();
    let chunk_builder_block_registry = block_registry.clone();
    let mut entities: HashMap<u32, game::Entity> = HashMap::new();
    std::thread::Builder::new()
        .name("chunk_builder".to_string())
        .stack_size(10000000)
        .spawn(move || loop {
            let (pos, data): (ChunkPosition, Vec<u8>) = chunk_builder_input_rx.recv().unwrap();
            let block_registry: BlockRegistry =
                { chunk_builder_block_registry.lock().unwrap().clone() };
            let mut decoder = libflate::zlib::Decoder::new(data.as_slice()).unwrap();
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
            let mut vertices: Vec<glwrappers::Vertex> = Vec::new();
            {
                for bx in 0..16i32 {
                    let x = bx as f32;
                    for by in 0..16i32 {
                        let y = by as f32;
                        for bz in 0..16i32 {
                            let z = bz as f32;
                            let block_id = blocks[bx as usize][by as usize][bz as usize];
                            let block = block_registry.get_block(block_id);
                            let position = BlockPosition {
                                x: bx,
                                y: by,
                                z: bz,
                            };
                            match &block.render_type {
                                BlockRenderType::Air => {}
                                BlockRenderType::Cube(north, south, right, left, up, down) => {
                                    for face in [
                                        Face::Front,
                                        Face::Back,
                                        Face::Up,
                                        Face::Down,
                                        Face::Left,
                                        Face::Right,
                                    ] {
                                        let face_offset = face.get_offset();
                                        let neighbor_pos = position + face_offset;
                                        let neighbor_side_full =
                                            if neighbor_pos.is_inside_origin_chunk() {
                                                block_registry
                                                    .get_block(
                                                        blocks[neighbor_pos.x as usize]
                                                            [neighbor_pos.y as usize]
                                                            [neighbor_pos.z as usize],
                                                    )
                                                    .is_face_full(&face.opposite())
                                            } else {
                                                false
                                            };
                                        let texture = match face {
                                            Face::Front => north,
                                            Face::Back => south,
                                            Face::Right => right,
                                            Face::Left => left,
                                            Face::Up => up,
                                            Face::Down => down,
                                        };
                                        if !neighbor_side_full {
                                            let face_vertices = face.get_vertices();
                                            let uv = texture.get_coords();
                                            vertices.push(glwrappers::Vertex {
                                                x: face_vertices[0].x + x,
                                                y: face_vertices[0].y + y,
                                                z: face_vertices[0].z + z,
                                                u: uv.0,
                                                v: uv.1,
                                                render_data: block.render_data,
                                            });
                                            vertices.push(glwrappers::Vertex {
                                                x: face_vertices[1].x + x,
                                                y: face_vertices[1].y + y,
                                                z: face_vertices[1].z + z,
                                                u: uv.2,
                                                v: uv.1,
                                                render_data: block.render_data,
                                            });
                                            vertices.push(glwrappers::Vertex {
                                                x: face_vertices[2].x + x,
                                                y: face_vertices[2].y + y,
                                                z: face_vertices[2].z + z,
                                                u: uv.2,
                                                v: uv.3,
                                                render_data: block.render_data,
                                            });
                                            vertices.push(glwrappers::Vertex {
                                                x: face_vertices[2].x + x,
                                                y: face_vertices[2].y + y,
                                                z: face_vertices[2].z + z,
                                                u: uv.2,
                                                v: uv.3,
                                                render_data: block.render_data,
                                            });
                                            vertices.push(glwrappers::Vertex {
                                                x: face_vertices[3].x + x,
                                                y: face_vertices[3].y + y,
                                                z: face_vertices[3].z + z,
                                                u: uv.0,
                                                v: uv.3,
                                                render_data: block.render_data,
                                            });
                                            vertices.push(glwrappers::Vertex {
                                                x: face_vertices[0].x + x,
                                                y: face_vertices[0].y + y,
                                                z: face_vertices[0].z + z,
                                                u: uv.0,
                                                v: uv.1,
                                                render_data: block.render_data,
                                            });
                                        }
                                    }
                                }
                                BlockRenderType::StaticModel(model, _, _, _, _, _, _) => {
                                    model.add_to_chunk_mesh(
                                        &mut vertices,
                                        block.render_data,
                                        BlockPosition {
                                            x: bx,
                                            y: by,
                                            z: bz,
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
            }
            chunk_builder_output_tx
                .send((pos, blocks, vertices))
                .unwrap();
        })
        .unwrap();
    'main_loop: loop {
        let render_start_time = Instant::now();
        let raycast_result = { raycast(&world, &camera, &block_registry.lock().unwrap()) };
        block_breaking_manager
            .set_target_block(raycast_result.map(|raycast| (raycast.0, raycast.2)));
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { timestamp: _ } => break 'main_loop,
                Event::MouseWheel {
                    timestamp: _,
                    window_id: _,
                    which: _,
                    x,
                    y,
                    direction: _,
                } => {
                    socket
                        .write_message(tungstenite::Message::Binary(
                            util::NetworkMessageC2S::MouseScroll(x, y).to_data(),
                        ))
                        .unwrap();
                }
                Event::MouseMotion {
                    timestamp: _,
                    window_id: _,
                    which: _,
                    mousestate: _,
                    x,
                    y,
                    xrel,
                    yrel,
                } => {
                    if !gui.on_mouse_move(x, y) {
                        let sensitivity = 0.5f32;
                        camera.update_orientation(
                            (-yrel as f32) * sensitivity,
                            (-xrel as f32) * sensitivity,
                        );
                    }
                }
                Event::MouseButtonDown {
                    timestamp: _,
                    window_id: _,
                    which: _,
                    mouse_btn,
                    clicks: _,
                    x: _,
                    y: _,
                } => {
                    if mouse_btn == MouseButton::Middle {
                        break 'main_loop;
                    }
                    if mouse_btn == MouseButton::Left {
                        if !gui.on_left_click(&mut socket) {
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
                            block_breaking_manager.set_left_click_held(true);
                        }
                    }
                    if mouse_btn == MouseButton::Right {
                        if let Some((position, _id, face)) = raycast_result {
                            if !gui.on_right_click() {
                                socket
                                    .write_message(tungstenite::Message::Binary(
                                        NetworkMessageC2S::RightClickBlock(
                                            position.x,
                                            position.y,
                                            position.z,
                                            face,
                                            camera.is_shifting(),
                                        )
                                        .to_data(),
                                    ))
                                    .unwrap();
                            }
                        }
                    }
                }
                Event::MouseButtonUp {
                    timestamp: _,
                    window_id: _,
                    which: _,
                    mouse_btn,
                    clicks: _,
                    x: _,
                    y: _,
                } => {
                    if mouse_btn == MouseButton::Left {
                        block_breaking_manager.set_left_click_held(false);
                    }
                }
                Event::KeyDown {
                    timestamp: _,
                    window_id: _,
                    keycode,
                    scancode: _,
                    keymod: _,
                    repeat,
                } => {
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
                Event::KeyUp {
                    timestamp: _,
                    window_id: _,
                    keycode,
                    scancode: _,
                    keymod: _,
                    repeat,
                } => {
                    keys_held.remove(&keycode.unwrap());
                    socket
                        .write_message(tungstenite::Message::Binary(
                            NetworkMessageC2S::Keyboard(keycode.unwrap() as i32, false, repeat)
                                .to_data(),
                        ))
                        .unwrap();
                }
                Event::Window {
                    timestamp: _,
                    window_id: _,
                    win_event,
                } => match win_event {
                    WindowEvent::Resized(width, height) => {
                        win_width = width as u32;
                        win_height = height as u32;
                        gui.size = (win_width, win_height);
                        unsafe {
                            ogl33::glViewport(0, 0, win_width as i32, win_height as i32);
                        }
                    }
                    _ => {}
                },
                _ => (),
            }
        }
        unsafe {
            ogl33::glClear(ogl33::GL_COLOR_BUFFER_BIT | ogl33::GL_DEPTH_BUFFER_BIT);

            let time = timer.ticks() as f32 / 1000f32;
            let delta_time = time - last_time;
            last_time = time;

            while let Ok(msg) = chunk_builder_output_rx.try_recv() {
                if let Some(chunk) = world.get_mut_chunk(msg.0) {
                    chunk.set_blocks_no_update(msg.1);
                    chunk.upload_vertices(msg.2);
                }
            }

            'message_loop: loop {
                match socket.read_message() {
                    Ok(msg) => match msg {
                        tungstenite::Message::Binary(msg) => {
                            let msg = msg.as_slice();
                            let message = NetworkMessageS2C::from_data(msg).unwrap();
                            match message {
                                NetworkMessageS2C::SetBlock(x, y, z, id) => {
                                    let position = BlockPosition { x, y, z };
                                    world.set_block(position, id).expect(
                                        format!("chunk not loaded at {x} {y} {z}").as_str(),
                                    );
                                }
                                NetworkMessageS2C::LoadChunk(x, y, z, blocks) => {
                                    world.load_chunk(ChunkPosition { x, y, z });
                                    chunk_builder_input_tx
                                        .send((ChunkPosition { x, y, z }, blocks))
                                        .unwrap();
                                }
                                NetworkMessageS2C::UnloadChunk(x, y, z) => {
                                    world.unload_chunk(ChunkPosition { x, y, z });
                                }
                                NetworkMessageS2C::AddEntity(
                                    entity_type,
                                    id,
                                    x,
                                    y,
                                    z,
                                    rotation,
                                ) => {
                                    entities.insert(
                                        id,
                                        game::Entity {
                                            entity_type,
                                            rotation,
                                            position: Position { x, y, z },
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
                                NetworkMessageS2C::InitializeContent(blocks, _entities, items) => {
                                    let mut guard = block_registry.lock().unwrap();
                                    let block_registry_blocks = &mut guard.blocks;
                                    for block in &blocks {
                                        match block.json["type"].as_str().unwrap() {
                                            "cube" => {
                                                block_registry_blocks.push(game::Block {
                                                    render_data: 0,
                                                    render_type: game::BlockRenderType::Cube(
                                                        texture_atlas
                                                            .get(
                                                                block.json["north"]
                                                                    .as_str()
                                                                    .unwrap(),
                                                            )
                                                            .clone(),
                                                        texture_atlas
                                                            .get(
                                                                block.json["south"]
                                                                    .as_str()
                                                                    .unwrap(),
                                                            )
                                                            .clone(),
                                                        texture_atlas
                                                            .get(
                                                                block.json["right"]
                                                                    .as_str()
                                                                    .unwrap(),
                                                            )
                                                            .clone(),
                                                        texture_atlas
                                                            .get(
                                                                block.json["left"]
                                                                    .as_str()
                                                                    .unwrap(),
                                                            )
                                                            .clone(),
                                                        texture_atlas
                                                            .get(block.json["up"].as_str().unwrap())
                                                            .clone(),
                                                        texture_atlas
                                                            .get(
                                                                block.json["down"]
                                                                    .as_str()
                                                                    .unwrap(),
                                                            )
                                                            .clone(),
                                                    ),
                                                });
                                            }
                                            "static" => {
                                                let texture = texture_atlas
                                                    .get(block.json["texture"].as_str().unwrap())
                                                    .clone();
                                                block_registry_blocks.push(Block {
                                                    render_data: 0,
                                                    render_type: BlockRenderType::StaticModel(
                                                        StaticBlockModel::new(
                                                            &json::parse(
                                                                std::fs::read_to_string(
                                                                    block.json["model"]
                                                                        .as_str()
                                                                        .unwrap()
                                                                        .to_string()
                                                                        + ".bbmodel",
                                                                )
                                                                .unwrap()
                                                                .as_str(),
                                                            )
                                                            .unwrap(),
                                                            &texture,
                                                        ),
                                                        block.json["north"].as_bool().unwrap(),
                                                        block.json["south"].as_bool().unwrap(),
                                                        block.json["right"].as_bool().unwrap(),
                                                        block.json["left"].as_bool().unwrap(),
                                                        block.json["up"].as_bool().unwrap(),
                                                        block.json["down"].as_bool().unwrap(),
                                                    ),
                                                })
                                            }
                                            _ => unreachable!(),
                                        }
                                    }
                                    let mut item_registry = item_registry.borrow_mut();
                                    for item in items {
                                        item_registry.push(item);
                                    }
                                }
                                NetworkMessageS2C::GuiData(data) => {
                                    gui.on_json_data(data);
                                }
                                NetworkMessageS2C::BlockBreakTimeResponse(id, time) => {
                                    block_breaking_manager.on_block_break_time_response(id, time);
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
            block_breaking_manager.tick(delta_time, &mut socket);
            camera.update_position(&keys_held, delta_time, &world);
            {
                window
                    .borrow_mut()
                    .set_title(
                        format!(
                            "BlockByte {:.1} {:.1} {:.1} {}",
                            camera.position.x,
                            camera.position.y,
                            camera.position.z,
                            last_frame_time
                        )
                        .as_str(),
                    )
                    .unwrap();
            }
            socket
                .write_message(tungstenite::Message::Binary(
                    NetworkMessageC2S::PlayerPosition(
                        camera.position.x,
                        camera.position.y,
                        camera.position.z,
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
            {
                world.render(&chunk_shader, (timer.ticks() as f32) / 1000f32);
            }

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
            for entity in &entities {
                model.render(
                    entity.1.position,
                    entity.1.rotation.to_radians(),
                    &model_shader,
                );
            }

            outline_shader.use_program();
            let projection_view_loc = outline_shader
                .get_uniform_location("projection_view\0")
                .expect("transform uniform not found");
            outline_shader.set_uniform_matrix(
                projection_view_loc,
                ultraviolet::projection::perspective_gl(
                    90f32.to_radians(),
                    (win_width as f32) / (win_height as f32),
                    0.01,
                    1000.,
                ) * camera.create_view_matrix(),
            );
            if let Some((position, id, _)) = raycast_result {
                ogl33::glDisable(ogl33::GL_DEPTH_TEST);
                outline_renderer.render(
                    id,
                    position,
                    &outline_shader,
                    &block_registry.lock().unwrap(),
                );
                ogl33::glEnable(ogl33::GL_DEPTH_TEST);
            }
            block_breaking_manager.render(&projection);
            gui.render(&gui_shader);
            {
                window.borrow().gl_swap_window();
            }
            last_frame_time =
                (1000000f64 / (render_start_time.elapsed().as_micros() as f64)) as u32 as f32;
        }
    }
}
fn pack_textures(textures: Vec<(&str, &std::path::Path)>) -> (TextureAtlas, RgbaImage) {
    let mut texture_map = std::collections::HashMap::new();
    let mut packer =
        texture_packer::TexturePacker::new_skyline(texture_packer::TexturePackerConfig {
            max_width: 256,
            max_height: 256,
            allow_rotation: false,
            texture_outlines: false,
            border_padding: 0,
            texture_padding: 0,
            trim: false,
            texture_extrusion: 0,
        });
    for (name, path) in textures {
        if let Ok(texture) = ImageImporter::import_from_file(path) {
            packer.pack_own(name, texture).unwrap();
        }
    }
    packer
        .pack_own(
            "missing",
            ImageImporter::import_from_file(Path::new("missing.png"))
                .expect("missing texture not found"),
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
    (
        TextureAtlas {
            missing_texture: texture_map.get("missing").unwrap().clone(),
            textures: texture_map,
        },
        exporter.to_rgba8(),
    )
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
pub fn raycast(
    world: &game::World,
    camera: &game::ClientPlayer,
    block_registry: &BlockRegistry,
) -> Option<(BlockPosition, u32, Face)> {
    //TODO: better algorithm
    let mut ray_pos = camera.get_eye().clone();
    let dir = camera.make_front().normalized() * 0.01;
    let mut last_pos = ray_pos.clone();
    for _ in 0..500 {
        let position = BlockPosition {
            x: ray_pos.x.floor() as i32,
            y: ray_pos.y.floor() as i32,
            z: ray_pos.z.floor() as i32,
        };
        if let Some(id) = world.get_block(position) {
            match &block_registry.get_block(id).render_type {
                BlockRenderType::Air => {}
                BlockRenderType::Cube(_, _, _, _, _, _) => {
                    let last_pos = last_pos.to_block_pos();
                    let mut least_diff_face = Face::Up;
                    for face in enum_iterator::all::<Face>() {
                        let offset = face.get_offset();
                        let diff = (position.x - last_pos.x + offset.x).abs()
                            + (position.y - last_pos.y + offset.y).abs()
                            + (position.z - last_pos.z + offset.z).abs();
                        if diff <= 1 {
                            least_diff_face = face;
                        }
                    }
                    return Some((position, id, least_diff_face));
                }
                BlockRenderType::StaticModel(model, _, _, _, _, _, _) => {
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
                            for face in enum_iterator::all::<Face>() {
                                let offset = face.get_offset();
                                let diff = (((x - cube.from.x) / size.x) - 0.5 + (offset.x as f32))
                                    .abs()
                                    + (((y - cube.from.y) / size.y) - 0.5 + (offset.y as f32))
                                        .abs()
                                    + (((z - cube.from.z) / size.z) - 0.5 + (offset.z as f32))
                                        .abs();
                                if diff <= least_diff {
                                    least_diff = diff;
                                    least_diff_face = face;
                                }
                            }
                            return Some((position, id, least_diff_face.opposite()));
                        }
                    }
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
    fn upload_static_model(&mut self, model: &StaticBlockModel, r: f32, g: f32, b: f32) {
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
    }
    pub fn render(
        &mut self,
        id: u32,
        position: BlockPosition,
        shader: &glwrappers::Shader,
        block_registry: &BlockRegistry,
    ) {
        self.vao.bind();
        self.vbo.bind();
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
        match (
            &block_registry.get_block(self.last_id).render_type,
            &block_registry.get_block(id).render_type,
        ) {
            (BlockRenderType::Air, BlockRenderType::Cube(_, _, _, _, _, _)) => {
                self.upload_cube(1., 0., 0.);
            }
            (BlockRenderType::Air, BlockRenderType::StaticModel(model, _, _, _, _, _, _)) => {
                self.upload_static_model(model, 1., 0., 0.);
            }
            (
                BlockRenderType::Cube(_, _, _, _, _, _),
                BlockRenderType::StaticModel(model, _, _, _, _, _, _),
            ) => {
                self.upload_static_model(model, 1., 0., 0.);
            }
            (
                BlockRenderType::StaticModel(_, _, _, _, _, _, _),
                BlockRenderType::Cube(_, _, _, _, _, _),
            ) => {
                self.upload_cube(1., 0., 0.);
            }
            (
                BlockRenderType::StaticModel(_, _, _, _, _, _, _),
                BlockRenderType::StaticModel(model, _, _, _, _, _, _),
            ) => {
                if self.last_id != id {
                    self.upload_static_model(model, 1., 0., 0.);
                }
            }
            _ => {}
        }
        unsafe {
            ogl33::glDrawArrays(ogl33::GL_LINES, 0, self.vertex_count);
        }
        self.last_id = id;
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
                std::mem::size_of::<glwrappers::BreakingVertex>()
                    .try_into()
                    .unwrap(),
                0 as *const _,
            );
            ogl33::glVertexAttribPointer(
                1,
                2,
                ogl33::GL_FLOAT,
                ogl33::GL_FALSE,
                std::mem::size_of::<glwrappers::BreakingVertex>()
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
                include_str!("shaders/block_breaking.vert").to_string(),
                include_str!("shaders/block_breaking.frag").to_string(),
            ),
            vao,
            vbo,
            breaking_textures,
        }
    }
    pub fn tick(&mut self, delta_time: f32, socket: &mut WebSocket<TcpStream>) {
        if let Some(target_block) = self.target_block {
            if self.key_down && self.breaking_animation.is_none() && !self.time_requested {
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
                let mut vertices: Vec<glwrappers::BreakingVertex> = Vec::new();
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
        }
        self.target_block = block;
    }
}
