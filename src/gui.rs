use std::{
    cell::RefCell,
    collections::HashMap,
    net::TcpStream,
    rc::Rc,
    sync::{Arc, Mutex},
};

use json::JsonValue;
use tungstenite::WebSocket;
use ultraviolet::Vec3;

use crate::{
    game::{self, AtlassedTexture, BlockRegistry},
    glwrappers,
    util::{ItemModel, ItemRenderData, NetworkMessageC2S},
    TextureAtlas,
};

pub struct GUIRenderer {
    vao: glwrappers::VertexArray,
    vbo: glwrappers::Buffer,
}
impl GUIRenderer {
    pub fn new() -> Self {
        let vao = glwrappers::VertexArray::new().expect("couldnt create vao for outline renderer");
        vao.bind();
        let vbo = glwrappers::Buffer::new(glwrappers::BufferType::Array)
            .expect("couldnt create vbo for chunk");
        vbo.bind();
        unsafe {
            ogl33::glVertexAttribPointer(
                0,
                2,
                ogl33::GL_FLOAT,
                ogl33::GL_FALSE,
                std::mem::size_of::<glwrappers::GuiVertex>()
                    .try_into()
                    .unwrap(),
                0 as *const _,
            );
            ogl33::glVertexAttribPointer(
                1,
                2,
                ogl33::GL_FLOAT,
                ogl33::GL_FALSE,
                std::mem::size_of::<glwrappers::GuiVertex>()
                    .try_into()
                    .unwrap(),
                std::mem::size_of::<[f32; 2]>() as *const _,
            );
            ogl33::glVertexAttribPointer(
                2,
                4,
                ogl33::GL_FLOAT,
                ogl33::GL_FALSE,
                std::mem::size_of::<glwrappers::GuiVertex>()
                    .try_into()
                    .unwrap(),
                std::mem::size_of::<[f32; 2 + 2]>() as *const _,
            );
            ogl33::glEnableVertexAttribArray(2);
            ogl33::glEnableVertexAttribArray(1);
            ogl33::glEnableVertexAttribArray(0);
        }
        GUIRenderer { vao, vbo }
    }
    pub fn render(
        &mut self,
        shader: &glwrappers::Shader,
        quads: Vec<GUIQuad>,
        width_multiplier: f32,
        height_multiplier: f32,
    ) {
        shader.use_program();
        self.vao.bind();
        self.vbo.bind();
        /*shader.set_uniform_matrix(
            shader
                .get_uniform_location("view")
                .expect("uniform view not found"),
            ultraviolet::Mat4::identity(),
        );*/
        let mut vertices: Vec<glwrappers::GuiVertex> = Vec::new();
        for quad in &quads {
            vertices.push([
                quad.x1 * width_multiplier,
                quad.y1 * height_multiplier,
                quad.u1,
                quad.v2,
                quad.color.r,
                quad.color.g,
                quad.color.b,
                quad.color.a,
            ]);
            vertices.push([
                quad.x2 * width_multiplier,
                quad.y2 * height_multiplier,
                quad.u2,
                quad.v2,
                quad.color.r,
                quad.color.g,
                quad.color.b,
                quad.color.a,
            ]);
            vertices.push([
                quad.x3 * width_multiplier,
                quad.y3 * height_multiplier,
                quad.u2,
                quad.v1,
                quad.color.r,
                quad.color.g,
                quad.color.b,
                quad.color.a,
            ]);
            vertices.push([
                quad.x3 * width_multiplier,
                quad.y3 * height_multiplier,
                quad.u2,
                quad.v1,
                quad.color.r,
                quad.color.g,
                quad.color.b,
                quad.color.a,
            ]);
            vertices.push([
                quad.x4 * width_multiplier,
                quad.y4 * height_multiplier,
                quad.u1,
                quad.v1,
                quad.color.r,
                quad.color.g,
                quad.color.b,
                quad.color.a,
            ]);
            vertices.push([
                quad.x1 * width_multiplier,
                quad.y1 * height_multiplier,
                quad.u1,
                quad.v2,
                quad.color.r,
                quad.color.g,
                quad.color.b,
                quad.color.a,
            ]);
        }
        self.vbo.upload_data(
            bytemuck::cast_slice(vertices.as_slice()),
            ogl33::GL_STREAM_DRAW,
        );
        unsafe {
            ogl33::glBlendFunc(ogl33::GL_SRC_ALPHA, ogl33::GL_ONE_MINUS_SRC_ALPHA);
            ogl33::glEnable(ogl33::GL_BLEND);
            ogl33::glDisable(ogl33::GL_DEPTH_TEST);
            ogl33::glDrawArrays(ogl33::GL_TRIANGLES, 0, (quads.len() * 6) as i32);
            ogl33::glEnable(ogl33::GL_DEPTH_TEST);
            ogl33::glDisable(ogl33::GL_BLEND);
        }
    }
}
pub struct GUIQuad {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    x3: f32,
    y3: f32,
    x4: f32,
    y4: f32,
    u1: f32,
    v1: f32,
    u2: f32,
    v2: f32,
    color: Color,
}
impl GUIQuad {
    pub fn new(x: f32, y: f32, w: f32, h: f32, texture: &AtlassedTexture, color: Color) -> GUIQuad {
        let uv = texture.get_coords();
        GUIQuad {
            x1: x,
            y1: y,
            x2: x + w,
            y2: y,
            x3: x + w,
            y3: y + h,
            x4: x,
            y4: y + h,
            u1: uv.0,
            v1: uv.1,
            u2: uv.2,
            v2: uv.3,
            color,
        }
    }
    pub fn new_uv(
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        uv: (f32, f32, f32, f32),
        color: Color,
    ) -> GUIQuad {
        GUIQuad {
            x1: x,
            y1: y,
            x2: x + w,
            y2: y,
            x3: x + w,
            y3: y + h,
            x4: x,
            y4: y + h,
            u1: uv.0,
            v1: uv.1,
            u2: uv.2,
            v2: uv.3,
            color,
        }
    }
}

pub enum GUIComponent {
    ImageComponent(
        f32,
        f32,
        AtlassedTexture,
        Color,
        Option<(f32, f32, f32, f32)>,
    ),
    TextComponent(f32, String, Color),
    SlotComponent(f32, Option<ItemSlot>, Color, bool),
}
impl GUIComponent {
    pub fn from_json(json: &JsonValue, texture_atlas: &TextureAtlas) -> GUIComponent {
        let json_color = &json["color"];
        let color = if json_color.is_null() {
            Color {
                r: 1.,
                g: 1.,
                b: 1.,
                a: 1.,
            }
        } else {
            Color {
                r: json_color[0].as_f32().unwrap(),
                g: json_color[1].as_f32().unwrap(),
                b: json_color[2].as_f32().unwrap(),
                a: json_color[3].as_f32().unwrap(),
            }
        };
        match json["element_type"].as_str().unwrap() {
            "image" => GUIComponent::ImageComponent(
                json["w"].as_f32().unwrap(),
                json["h"].as_f32().unwrap(),
                texture_atlas.get(json["texture"].as_str().unwrap()).clone(),
                color,
                None,
            ),
            "text" => GUIComponent::TextComponent(1., String::new(), color),
            "slot" => {
                let json_slot = &json["item"];
                let item = if json_slot.is_null() {
                    None
                } else {
                    Some(ItemSlot {
                        item: json_slot["item"].as_u32().unwrap(),
                        count: json_slot["count"].as_u16().unwrap(),
                    })
                };
                GUIComponent::SlotComponent(
                    1.,
                    item,
                    color,
                    json["background"].as_bool().unwrap_or(true),
                )
            }
            text => panic!("unknown element type {}", text),
        }
    }
    pub fn set_data(&mut self, data_type: &str, json: &json::JsonValue) {
        match self {
            Self::ImageComponent(w, h, texture, color, slice) => match data_type {
                "color" => {
                    let json_color = &json["color"];
                    *color = Color {
                        r: json_color[0].as_f32().unwrap(),
                        g: json_color[1].as_f32().unwrap(),
                        b: json_color[2].as_f32().unwrap(),
                        a: json_color[3].as_f32().unwrap(),
                    };
                }
                "dimension" => {
                    let json_dimensions = &json["dimension"];
                    *w = json_dimensions[0].as_f32().unwrap();
                    *h = json_dimensions[1].as_f32().unwrap();
                }
                "slice" => {
                    let json_slice = &json["slice"];
                    *slice = if json_slice.is_null() {
                        None
                    } else {
                        Some((
                            json_slice[0].as_f32().unwrap(),
                            json_slice[1].as_f32().unwrap(),
                            json_slice[2].as_f32().unwrap(),
                            json_slice[3].as_f32().unwrap(),
                        ))
                    };
                }
                _ => {}
            },
            Self::TextComponent(scale, text, color) => match data_type {
                "color" => {
                    let json_color = &json["color"];
                    *color = Color {
                        r: json_color[0].as_f32().unwrap(),
                        g: json_color[1].as_f32().unwrap(),
                        b: json_color[2].as_f32().unwrap(),
                        a: json_color[3].as_f32().unwrap(),
                    };
                }
                "text" => {
                    *text = json["text"].as_str().unwrap().to_string();
                }
                _ => {}
            },
            GUIComponent::SlotComponent(size, slot, color, background) => match data_type {
                "color" => {
                    let json_color = &json["color"];
                    *color = Color {
                        r: json_color[0].as_f32().unwrap(),
                        g: json_color[1].as_f32().unwrap(),
                        b: json_color[2].as_f32().unwrap(),
                        a: json_color[3].as_f32().unwrap(),
                    };
                }
                "item" => {
                    let json_slot = &json["item"];
                    *slot = if json_slot.is_null() {
                        None
                    } else {
                        Some(ItemSlot {
                            item: json_slot["item"].as_u32().unwrap(),
                            count: json_slot["count"].as_u16().unwrap(),
                        })
                    }
                }
                "background" => {
                    *background = json["background"].as_bool().unwrap();
                }
                _ => {}
            },
        }
    }
    pub fn add_quads(
        &self,
        quads: &mut Vec<GUIQuad>,
        text_renderer: &TextRenderer,
        texture_atlas: &TextureAtlas,
        item_renderer: &HashMap<u32, ItemRenderData>,
        block_registry: &BlockRegistry,
        x: f32,
        y: f32,
    ) {
        match self {
            Self::ImageComponent(w, h, texture, color, slice) => match slice {
                Some(slice) => {
                    let uv0 = texture.map_uv((slice.0, slice.1));
                    let uv1 = texture.map_uv((slice.2, slice.3));
                    quads.push(GUIQuad::new_uv(
                        x,
                        y,
                        *w,
                        *h,
                        (uv0.0, uv0.1, uv1.0, uv1.1),
                        *color,
                    ));
                }
                None => {
                    quads.push(GUIQuad::new(x, y, *w, *h, &texture, *color));
                }
            },
            Self::TextComponent(scale, text, color) => {
                let mut y_cnt = 0;
                for text in text.split('\n') {
                    let mut x_cnt = 0;
                    for ch in text.bytes() {
                        if ch != (' ' as u8) {
                            let i = x_cnt as f32;
                            let coords = text_renderer.resolve_char(ch);
                            let width = 0.05f32 * scale;
                            let kerning = 0.01f32 * scale;
                            let line_separation = 0.01f32 * scale;
                            let height = 0.07f32 * scale;
                            let quad_x = x + (i * (width + kerning));
                            let quad_y = y - ((y_cnt as f32) * (height + line_separation));
                            quads.push(GUIQuad::new_uv(
                                quad_x,
                                quad_y,
                                width,
                                height,
                                (coords.0, coords.1, coords.2, coords.3),
                                *color,
                            ));
                        }
                        x_cnt += 1;
                    }
                    y_cnt += 1;
                }
            }
            Self::SlotComponent(size, item, color, background) => {
                let size = size * 0.1;
                let border = size * 0.1;
                if *background {
                    GUIComponent::ImageComponent(
                        size + (2. * border),
                        size + (2. * border),
                        texture_atlas.get("slot").clone(),
                        *color,
                        None,
                    )
                    .add_quads(
                        quads,
                        text_renderer,
                        texture_atlas,
                        item_renderer,
                        block_registry,
                        x - border,
                        y - border,
                    );
                }
                if let Some(slot) = item {
                    let item_render_data = item_renderer.get(&slot.item).unwrap();
                    match &item_render_data.model {
                        ItemModel::Texture(texture) => {
                            GUIComponent::ImageComponent(
                                size,
                                size,
                                texture_atlas.get(texture).clone(),
                                Color {
                                    r: 1.,
                                    g: 1.,
                                    b: 1.,
                                    a: 1.,
                                },
                                None,
                            )
                            .add_quads(
                                quads,
                                text_renderer,
                                texture_atlas,
                                item_renderer,
                                block_registry,
                                x,
                                y,
                            );
                        }
                        ItemModel::Block(block_id) => {
                            let block = block_registry.get_block(*block_id);
                            match block.render_type {
                                game::BlockRenderType::Air => {}
                                game::BlockRenderType::Cube(_, north, _, right, _, up, _) => {
                                    let top_texture = up.get_coords();
                                    let front_texture = north.get_coords();
                                    let right_texture = right.get_coords();
                                    let middle_x = size * 13. / 26.;
                                    let middle_y = size * 11. / 26.;
                                    quads.push(GUIQuad {
                                        x1: x,
                                        y1: y + (size / 6. * 5.),
                                        x2: x + middle_x,
                                        y2: y + size,
                                        x3: x + size,
                                        y3: y + (size / 6. * 5.),
                                        x4: x + middle_x,
                                        y4: y + middle_y,
                                        color: Color {
                                            r: 1.,
                                            g: 1.,
                                            b: 1.,
                                            a: 1.,
                                        },
                                        u1: top_texture.0,
                                        v1: top_texture.1,
                                        u2: top_texture.2,
                                        v2: top_texture.3,
                                    });
                                    quads.push(GUIQuad {
                                        x1: x + middle_x,
                                        y1: y + middle_y,
                                        x2: x + size,
                                        y2: y + (size * 5. / 6.),
                                        x3: x + (size * 23. / 25.),
                                        y3: y + (size * 7.5 / 25.),
                                        x4: x + middle_x,
                                        y4: y,
                                        color: Color {
                                            r: 1.,
                                            g: 1.,
                                            b: 1.,
                                            a: 1.,
                                        },
                                        u1: front_texture.0,
                                        v1: front_texture.3,
                                        u2: front_texture.2,
                                        v2: front_texture.1,
                                    });
                                    quads.push(GUIQuad {
                                        x1: x,
                                        y1: y + (size * 5. / 6.),
                                        x2: x + middle_x,
                                        y2: y + middle_y,
                                        x3: x + middle_x,
                                        y3: y,
                                        x4: x + (size * 2. / 25.),
                                        y4: y + (size * 7.5 / 25.),
                                        color: Color {
                                            r: 1.,
                                            g: 1.,
                                            b: 1.,
                                            a: 1.,
                                        },
                                        u1: right_texture.0,
                                        v1: right_texture.3,
                                        u2: right_texture.2,
                                        v2: right_texture.1,
                                    });
                                }
                                game::BlockRenderType::StaticModel(_, _, _, _, _, _, _, _) => {
                                    todo!()
                                }
                            }
                        }
                    }

                    if slot.count > 1 {
                        let text = GUIComponent::TextComponent(
                            size * 5.,
                            slot.count.to_string(),
                            Color {
                                r: 1.,
                                g: 1.,
                                b: 1.,
                                a: 1.,
                            },
                        );
                        text.add_quads(
                            quads,
                            text_renderer,
                            texture_atlas,
                            item_renderer,
                            block_registry,
                            x + size - text.get_width(),
                            y + text.get_height(),
                        );
                    }
                }
            }
        }
    }
    pub fn get_width(&self) -> f32 {
        match self {
            Self::ImageComponent(w, _, _, _, _) => *w,
            Self::TextComponent(scale, text, _) => text
                .split('\n')
                .map(|t| (t.len() as f32) * 0.06 * scale)
                .reduce(f32::max)
                .unwrap_or(0.),
            Self::SlotComponent(size, _, _, border) => {
                let size = size * 0.1;
                let border = if *border { size * 0.1 } else { 0. };
                size + (2. * border)
            }
        }
    }
    pub fn get_height(&self) -> f32 {
        match self {
            Self::ImageComponent(_, h, _, _, _) => *h,
            Self::TextComponent(scale, text, _) => {
                text.split('\n').count() as f32 * 0.08f32 * scale
            }
            Self::SlotComponent(_, _, _, _) => self.get_width(),
        }
    }
}
pub struct GUI<'a> {
    renderer: GUIRenderer,
    font_renderer: TextRenderer,
    item_renderer: &'a HashMap<u32, ItemRenderData>,
    slots: Vec<Option<ItemSlot>>,
    texture_atlas: TextureAtlas,
    elements: HashMap<String, GUIElement>,
    cursor: Option<(GUIComponent, f32, f32)>,
    sdl: &'a sdl2::Sdl,
    mouse_locked: bool,
    pub size: (u32, u32),
    window: &'a RefCell<sdl2::video::Window>,
    block_registry: &'a game::BlockRegistry,
    pub gui_scale: f32,
}
impl<'a> GUI<'a> {
    pub fn new(
        text_renderer: TextRenderer,
        item_renderer: &'a HashMap<u32, ItemRenderData>,
        texture_atlas: TextureAtlas,
        sdl: &'a sdl2::Sdl,
        size: (u32, u32),
        window: &'a RefCell<sdl2::video::Window>,
        block_registry: &'a game::BlockRegistry,
    ) -> Self {
        Self {
            cursor: None,
            renderer: GUIRenderer::new(),
            font_renderer: text_renderer,
            item_renderer,
            slots: vec![None; 9],
            texture_atlas,
            elements: HashMap::new(),
            sdl,
            mouse_locked: false,
            size,
            window,
            block_registry,
            gui_scale: 1.5,
        }
    }
    pub fn on_json_data(&mut self, data: JsonValue) {
        match data["type"].as_str().unwrap() {
            "setElement" => {
                let id = data["id"].as_str().unwrap().to_string();
                match id.as_str() {
                    "cursor" => {
                        let component = GUIComponent::from_json(&data, &self.texture_atlas);
                        if let Some(cursor) = &mut self.cursor {
                            cursor.0 = component;
                        } else {
                            self.cursor = Some((component, 0., 0.));
                        }
                    }
                    _ => {
                        if !data["element_type"].is_null() {
                            let component = GUIComponent::from_json(&data, &self.texture_atlas);
                            let element = GUIElement {
                                component,
                                x: data["x"].as_f32().unwrap(),
                                y: data["y"].as_f32().unwrap(),
                                z: data["z"].as_i32().unwrap_or(0),
                            };
                            self.elements.insert(id, element);
                        } else {
                            self.elements.remove(&id);
                        }
                    }
                }
            }
            "editElement" => {
                let id = data["id"].as_str().unwrap().to_string();
                match id.as_str() {
                    "cursor" => {
                        if let Some(cursor) = &mut self.cursor {
                            let data_type = data["data_type"].as_str().unwrap();
                            if data_type == "position" {
                                let position = &data["position"];
                                let x = position[0].as_f32().unwrap();
                                let y = position[1].as_f32().unwrap();
                                cursor.1 = x;
                                cursor.2 = y;
                                self.sdl.mouse().warp_mouse_in_window(
                                    &self.window.borrow(),
                                    (x * self.size.0 as f32) as i32,
                                    (y * self.size.1 as f32) as i32,
                                );
                            } else {
                                cursor.0.set_data(data_type, &data);
                            }
                        }
                    }
                    _ => {
                        if let Some(element) = self.elements.get_mut(&id) {
                            let data_type = data["data_type"].as_str().unwrap();
                            if data_type == "position" {
                                let position = &data["position"];
                                element.x = position[0].as_f32().unwrap();
                                element.y = position[1].as_f32().unwrap();
                            } else {
                                element.component.set_data(data_type, &data);
                            }
                        }
                    }
                }
            }
            "setCursorLock" => {
                let lock = data["lock"].as_bool().unwrap();
                self.sdl.mouse().set_relative_mouse_mode(lock);
                self.mouse_locked = lock;
                if let Some(cursor) = &mut self.cursor {
                    cursor.1 = 0.;
                    cursor.2 = 0.;
                    self.sdl.mouse().warp_mouse_in_window(
                        &self.window.borrow(),
                        (self.size.0 / 2) as i32,
                        (self.size.1 / 2) as i32,
                    );
                }
            }
            "removeContainer" => {
                let container = data["container"].as_str().unwrap();
                self.elements
                    .drain_filter(|id, _| id.starts_with(container));
            }
            _ => {}
        }
    }
    fn to_quad_list(&self, x: f32, y: f32, z: f32, fps: u32) -> Vec<GUIQuad> {
        let mut quads = Vec::new();
        let mut elements: Vec<&GUIElement> = self.elements.values().collect();
        elements.sort_by(|a, b| a.z.cmp(&b.z));
        for element in &elements {
            element.component.add_quads(
                &mut quads,
                &self.font_renderer,
                &self.texture_atlas,
                self.item_renderer,
                self.block_registry,
                element.x,
                element.y,
            );
        }
        if let Some(cursor) = &self.cursor {
            cursor.0.add_quads(
                &mut quads,
                &self.font_renderer,
                &self.texture_atlas,
                self.item_renderer,
                self.block_registry,
                cursor.1 - (cursor.0.get_width() / 2.),
                cursor.2 - (cursor.0.get_height() / 2.),
            );
            for element in &elements {
                if element.x <= cursor.1
                    && element.x + element.component.get_width() >= cursor.1
                    && element.y <= cursor.2
                    && element.y + element.component.get_height() >= cursor.2
                {
                    if let GUIComponent::SlotComponent(_, item, _, background) = &element.component
                    {
                        if *background {
                            if let Some(item) = item {
                                GUIComponent::TextComponent(
                                    1.,
                                    self.item_renderer.get(&item.item).unwrap().name.clone(),
                                    Color {
                                        r: 1.,
                                        g: 1.,
                                        b: 1.,
                                        a: 1.,
                                    },
                                )
                                .add_quads(
                                    &mut quads,
                                    &self.font_renderer,
                                    &self.texture_atlas,
                                    self.item_renderer,
                                    self.block_registry,
                                    cursor.1,
                                    cursor.2,
                                )
                            }
                        }
                    }
                }
            }
        }
        GUIComponent::TextComponent(
            1.,
            format!("x:{:.2} y:{:.2} z:{:.2} fps: {}", x, y, z, fps),
            Color {
                r: 0.,
                g: 0.,
                b: 0.,
                a: 1.,
            },
        )
        .add_quads(
            &mut quads,
            &self.font_renderer,
            &self.texture_atlas,
            &self.item_renderer,
            &self.block_registry,
            -1.1,
            0.4,
        );
        quads
    }
    pub fn on_mouse_move(&mut self, x: i32, y: i32) -> bool {
        if !self.mouse_locked {
            if let Some(cursor) = &mut self.cursor {
                let half_width = (self.size.0 as f32) / 2.;
                let half_height = (self.size.1 as f32) / 2.;
                cursor.1 = (((x as f32) - half_width) / half_width)
                    / (self.size.1 as f32 / self.size.0 as f32)
                    / self.gui_scale;
                cursor.2 = (-((y as f32) - half_height) / half_height) / self.gui_scale;
            }
        }
        !self.mouse_locked
    }
    pub fn on_left_click(&mut self, socket: &mut WebSocket<TcpStream>, shifting: bool) -> bool {
        if !self.mouse_locked {
            if let Some(cursor) = &self.cursor {
                let mut id = None;
                for element in &self.elements {
                    if element.1.x <= cursor.1
                        && element.1.x + element.1.component.get_width() >= cursor.1
                        && element.1.y <= cursor.2
                        && element.1.y + element.1.component.get_height() >= cursor.2
                    {
                        id = Some(element.0.clone());
                    }
                }
                if let Some(id) = id {
                    socket
                        .write_message(tungstenite::Message::Binary(
                            NetworkMessageC2S::GuiClick(
                                id,
                                crate::util::MouseButton::LEFT,
                                shifting,
                            )
                            .to_data(),
                        ))
                        .unwrap();
                }
            }
        }
        !self.mouse_locked
    }
    pub fn on_right_click(&mut self) -> bool {
        !self.mouse_locked
    }
    pub fn on_mouse_scroll(
        &mut self,
        socket: &mut WebSocket<TcpStream>,
        x: i32,
        y: i32,
        shifting: bool,
    ) -> bool {
        if !self.mouse_locked {
            if let Some(cursor) = &self.cursor {
                let mut id = None;
                for element in &self.elements {
                    if element.1.x <= cursor.1
                        && element.1.x + element.1.component.get_width() >= cursor.1
                        && element.1.y <= cursor.2
                        && element.1.y + element.1.component.get_height() >= cursor.2
                    {
                        id = Some(element.0.clone());
                    }
                }
                if let Some(id) = id {
                    socket
                        .write_message(tungstenite::Message::Binary(
                            NetworkMessageC2S::GuiScroll(id, x, y, shifting).to_data(),
                        ))
                        .unwrap();
                }
            }
        }
        !self.mouse_locked
    }
    pub fn render(&mut self, shader: &glwrappers::Shader, player_pos: &Vec3, fps: u32) {
        self.renderer.render(
            shader,
            self.to_quad_list(player_pos.x, player_pos.y, player_pos.z, fps),
            (self.size.1 as f32 / self.size.0 as f32) * self.gui_scale,
            self.gui_scale,
        );
    }
}
pub struct TextRenderer {
    pub texture: game::AtlassedTexture,
}
impl TextRenderer {
    pub fn resolve_char(&self, ch: u8) -> (f32, f32, f32, f32) {
        let ch = ch.to_ascii_uppercase();
        let index = if ch.is_ascii_uppercase() {
            ch - ('A' as u8)
        } else if ch.is_ascii_digit() {
            ch - ('0' as u8) + 27
        } else if ch == ':' as u8 {
            37
        } else if ch == '.' as u8 {
            38
        } else if ch == '-' as u8 {
            39
        } else {
            26
        };
        let index = index as f32;
        let uv1 = self.texture.map((index * 5f32, 0f32));
        let uv2 = self.texture.map(((index + 1f32) * 5f32, 7f32));
        (uv1.0, uv1.1, uv2.0, uv2.1)
    }
}

pub struct GUIElement {
    pub component: GUIComponent,
    pub x: f32,
    pub y: f32,
    pub z: i32,
}

#[derive(Clone, Copy)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[derive(Clone, Copy)]
pub struct ItemSlot {
    item: u32,
    count: u16,
}
