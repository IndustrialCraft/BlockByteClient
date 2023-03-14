use std::{cell::RefCell, collections::HashMap, net::TcpStream, rc::Rc};

use json::JsonValue;
use tungstenite::WebSocket;

use crate::{
    game::{self, AtlassedTexture},
    glwrappers,
    util::{ItemRenderData, NetworkMessageC2S},
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
    pub fn render(&mut self, shader: &glwrappers::Shader, quads: Vec<GUIQuad>) {
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
                quad.x,
                quad.y,
                quad.u1,
                quad.v1,
                quad.color.r,
                quad.color.g,
                quad.color.b,
                quad.color.a,
            ]);
            vertices.push([
                quad.x + quad.w,
                quad.y,
                quad.u2,
                quad.v1,
                quad.color.r,
                quad.color.g,
                quad.color.b,
                quad.color.a,
            ]);
            vertices.push([
                quad.x + quad.w,
                quad.y + quad.h,
                quad.u2,
                quad.v2,
                quad.color.r,
                quad.color.g,
                quad.color.b,
                quad.color.a,
            ]);
            vertices.push([
                quad.x + quad.w,
                quad.y + quad.h,
                quad.u2,
                quad.v2,
                quad.color.r,
                quad.color.g,
                quad.color.b,
                quad.color.a,
            ]);
            vertices.push([
                quad.x,
                quad.y + quad.h,
                quad.u1,
                quad.v2,
                quad.color.r,
                quad.color.g,
                quad.color.b,
                quad.color.a,
            ]);
            vertices.push([
                quad.x,
                quad.y,
                quad.u1,
                quad.v1,
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
    x: f32,
    y: f32,
    w: f32,
    h: f32,
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
            x,
            y,
            w,
            h,
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
    SlotComponent(f32, Option<ItemSlot>, Color),
}
impl GUIComponent {
    pub fn from_json(
        json: &JsonValue,
        texture_atlas: &HashMap<String, AtlassedTexture>,
    ) -> GUIComponent {
        match json["element_type"].as_str().unwrap() {
            "image" => GUIComponent::ImageComponent(
                json["w"].as_f32().unwrap(),
                json["h"].as_f32().unwrap(),
                texture_atlas
                    .get(json["texture"].as_str().unwrap())
                    .unwrap()
                    .clone(),
                Color {
                    r: 1.,
                    g: 1.,
                    b: 1.,
                    a: 1.,
                },
                None,
            ),
            "text" => GUIComponent::TextComponent(
                1.,
                String::new(),
                Color {
                    r: 1.,
                    g: 1.,
                    b: 1.,
                    a: 1.,
                },
            ),
            "slot" => GUIComponent::SlotComponent(
                1.,
                None,
                Color {
                    r: 1.,
                    g: 1.,
                    b: 1.,
                    a: 1.,
                },
            ),
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
            GUIComponent::SlotComponent(size, slot, color) => match data_type {
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
                _ => {}
            },
        }
    }
    pub fn add_quads(
        &self,
        quads: &mut Vec<GUIQuad>,
        text_renderer: &TextRenderer,
        texture_atlas: &HashMap<String, AtlassedTexture>,
        item_renderer: &Rc<RefCell<Vec<ItemRenderData>>>,
        x: f32,
        y: f32,
    ) {
        match self {
            Self::ImageComponent(w, h, texture, color, slice) => match slice {
                Some(slice) => {
                    let uv0 = texture.map((slice.0, slice.1));
                    let uv1 = texture.map((slice.2, slice.3));
                    quads.push(GUIQuad {
                        x,
                        y,
                        w: *w,
                        h: *h,
                        color: *color,
                        u1: uv0.0,
                        v1: uv0.1,
                        u2: uv1.0,
                        v2: uv1.1,
                    });
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
                            quads.push(GUIQuad {
                                x: x + (i * (width + kerning)),
                                y: y - ((y_cnt as f32) * (height + line_separation)),
                                w: width,
                                h: height,
                                u1: coords.0,
                                v2: coords.1,
                                u2: coords.2,
                                v1: coords.3,
                                color: *color,
                            });
                        }
                        x_cnt += 1;
                    }
                    y_cnt += 1;
                }
            }
            Self::SlotComponent(size, item, color) => {
                let size = size * 0.1;
                let border = size * 0.1;
                GUIComponent::ImageComponent(
                    size + (2. * border),
                    size + (2. * border),
                    texture_atlas.get("slot").unwrap().clone(),
                    *color,
                    None,
                )
                .add_quads(
                    quads,
                    text_renderer,
                    texture_atlas,
                    item_renderer,
                    x - border,
                    y - border,
                );
                if let Some(slot) = item {
                    GUIComponent::ImageComponent(
                        size,
                        size,
                        texture_atlas
                            .get(
                                &item_renderer
                                    .borrow()
                                    .get(slot.item as usize)
                                    .unwrap()
                                    .texture,
                            )
                            .unwrap()
                            .clone(),
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
                        x,
                        y,
                    );
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
            Self::SlotComponent(size, _, _) => {
                let size = size * 0.1;
                let border = size * 0.1;
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
            Self::SlotComponent(_, _, _) => self.get_width(),
        }
    }
}
pub struct GUI<'a> {
    renderer: GUIRenderer,
    font_renderer: TextRenderer,
    item_renderer: Rc<RefCell<Vec<ItemRenderData>>>,
    slots: Vec<Option<ItemSlot>>,
    texture_atlas: HashMap<String, AtlassedTexture>,
    elements: HashMap<String, GUIElement>,
    cursor: Option<(AtlassedTexture, f32, f32, f32, f32)>,
    sdl: &'a sdl2::Sdl,
    mouse_locked: bool,
    pub size: (u32, u32),
    window: &'a RefCell<sdl2::video::Window>,
}
impl<'a> GUI<'a> {
    pub fn new(
        text_renderer: TextRenderer,
        item_renderer: Rc<RefCell<Vec<ItemRenderData>>>,
        texture_atlas: HashMap<String, AtlassedTexture>,
        sdl: &'a sdl2::Sdl,
        size: (u32, u32),
        window: &'a RefCell<sdl2::video::Window>,
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
        }
    }
    pub fn on_json_data(&mut self, data: JsonValue) {
        match data["type"].as_str().unwrap() {
            "setElement" => {
                let id = data["id"].as_str().unwrap().to_string();
                if !data["element_type"].is_null() {
                    let component = GUIComponent::from_json(&data, &self.texture_atlas);
                    let element = GUIElement {
                        component,
                        x: data["x"].as_f32().unwrap(),
                        y: data["y"].as_f32().unwrap(),
                    };
                    self.elements.insert(id, element);
                } else {
                    self.elements.remove(&id);
                }
            }
            "editElement" => {
                let id = data["id"].as_str().unwrap().to_string();
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
            "setCursor" => {
                let texture = &data["texture"];
                if texture.is_null() {
                    self.cursor = None;
                } else {
                    self.cursor = Some((
                        self.texture_atlas
                            .get(texture.as_str().unwrap())
                            .unwrap()
                            .clone(),
                        0.,
                        0.,
                        data["width"].as_f32().unwrap(),
                        data["height"].as_f32().unwrap(),
                    ));
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
    fn to_quad_list(&self) -> Vec<GUIQuad> {
        let mut quads = Vec::new();
        for element in &self.elements {
            element.1.component.add_quads(
                &mut quads,
                &self.font_renderer,
                &self.texture_atlas,
                &self.item_renderer,
                element.1.x,
                element.1.y,
            );
        }
        if let Some(cursor) = &self.cursor {
            quads.push(GUIQuad::new(
                cursor.1 - (cursor.3 / 2.),
                cursor.2 - (cursor.4 / 2.),
                cursor.3,
                cursor.4,
                &cursor.0,
                Color {
                    r: 1.,
                    g: 1.,
                    b: 1.,
                    a: 1.,
                },
            ));
        }
        quads
    }
    pub fn on_mouse_move(&mut self, x: i32, y: i32) -> bool {
        if !self.mouse_locked {
            if let Some(cursor) = &mut self.cursor {
                let half_width = (self.size.0 as f32) / 2.;
                let half_height = (self.size.1 as f32) / 2.;
                cursor.1 = ((x as f32) - half_width) / half_width;
                cursor.2 = -((y as f32) - half_height) / half_height;
            }
        }
        !self.mouse_locked
    }
    pub fn on_left_click(&mut self, socket: &mut WebSocket<TcpStream>) -> bool {
        if !self.mouse_locked {
            if let Some(cursor) = self.cursor {
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
                            NetworkMessageC2S::GuiClick(id, crate::util::MouseButton::LEFT)
                                .to_data(),
                        ))
                        .unwrap();
                }
            }
        }
        !self.mouse_locked
    }
    pub fn render(&mut self, shader: &glwrappers::Shader) {
        self.renderer.render(shader, self.to_quad_list());
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
