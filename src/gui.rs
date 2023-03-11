use std::{cell::RefCell, collections::HashMap, rc::Rc};

use json::JsonValue;

use crate::{
    game::{self, AtlassedTexture},
    glwrappers,
    util::ItemRenderData,
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
    ImageComponent(f32, f32, AtlassedTexture, Color),
    TextComponent(f32, String, Color),
    SlotComponent(f32, Option<ItemSlot>, Color),
}
impl GUIComponent {
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
            Self::ImageComponent(w, h, texture, color) => {
                quads.push(GUIQuad::new(x, y, *w, *h, &texture, *color));
            }
            Self::TextComponent(scale, text, color) => {
                let mut y_cnt = 1;
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
            Self::ImageComponent(w, _, _, _) => *w,
            Self::TextComponent(scale, text, _) => text
                .split('\n')
                .map(|t| (t.len() as f32) * 0.06 * scale)
                .reduce(f32::max)
                .unwrap_or(0.),
            Self::SlotComponent(_, _, _) => todo!(),
        }
    }
    pub fn get_height(&self) -> f32 {
        match self {
            Self::ImageComponent(_, h, _, _) => *h,
            Self::TextComponent(scale, text, _) => {
                text.split('\n').count() as f32 * 0.08f32 * scale
            }
            Self::SlotComponent(_, _, _) => todo!(),
        }
    }
}
pub struct GUI {
    cursor: (f32, f32),
    renderer: GUIRenderer,
    font_renderer: TextRenderer,
    item_renderer: Rc<RefCell<Vec<ItemRenderData>>>,
    slots: Vec<Option<ItemSlot>>,
    texture_atlas: HashMap<String, AtlassedTexture>,
    pub selected_slot: u8,
}
impl GUI {
    pub fn new(
        text_renderer: TextRenderer,
        item_renderer: Rc<RefCell<Vec<ItemRenderData>>>,
        texture_atlas: HashMap<String, AtlassedTexture>,
    ) -> Self {
        Self {
            cursor: (0., 0.),
            renderer: GUIRenderer::new(),
            font_renderer: text_renderer,
            item_renderer,
            slots: vec![None; 9],
            texture_atlas,
            selected_slot: 0,
        }
    }
    pub fn on_json_data(&mut self, data: JsonValue) {
        match data["type"].as_str().unwrap() {
            "setItem" => {
                let slot = data["slot"].as_u32().unwrap();
                let item = data["item"].as_u32().unwrap();
                let count = data["count"].as_u16().unwrap();
                self.slots[slot as usize] = Some(ItemSlot { item, count });
            }
            "removeItem" => {
                let slot = data["slot"].as_u32().unwrap();
                self.slots[slot as usize] = None;
            }
            "selectSlot" => {
                self.selected_slot = data["slot"].as_u8().unwrap();
            }
            _ => {}
        }
    }
    fn to_quad_list(&self) -> Vec<GUIQuad> {
        let mut quads = Vec::new();
        GUIComponent::ImageComponent(
            0.1,
            0.1,
            self.texture_atlas.get("cursor").unwrap().clone(),
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
            &self.item_renderer,
            -0.05,
            -0.05,
        );
        for i in 0..9u8 {
            let x = ((i as f32) * 0.13) - 0.7;
            let y = -0.5;
            GUIComponent::SlotComponent(
                1.,
                self.slots.get(i as usize).unwrap().clone(),
                if i == self.selected_slot {
                    Color {
                        r: 1.,
                        g: 0.,
                        b: 0.,
                        a: 1.,
                    }
                } else {
                    Color {
                        r: 1.,
                        b: 1.,
                        g: 1.,
                        a: 1.,
                    }
                },
            )
            .add_quads(
                &mut quads,
                &self.font_renderer,
                &self.texture_atlas,
                &self.item_renderer,
                x,
                y,
            );
        }
        quads
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

#[derive(Clone, Copy)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[derive(Clone, Copy)]
struct ItemSlot {
    item: u32,
    count: u16,
}
