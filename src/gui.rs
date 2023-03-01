use crate::{game::AtlassedTexture, glwrappers};

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
            vertices.push([quad.x, quad.y, quad.u1, quad.v1]);
            vertices.push([quad.x + quad.w, quad.y, quad.u2, quad.v1]);
            vertices.push([quad.x + quad.w, quad.y + quad.h, quad.u2, quad.v2]);
            vertices.push([quad.x + quad.w, quad.y + quad.h, quad.u2, quad.v2]);
            vertices.push([quad.x, quad.y + quad.h, quad.u1, quad.v2]);
            vertices.push([quad.x, quad.y, quad.u1, quad.v1]);
        }
        self.vbo.upload_data(
            bytemuck::cast_slice(vertices.as_slice()),
            ogl33::GL_STREAM_DRAW,
        );
        unsafe {
            ogl33::glDisable(ogl33::GL_DEPTH_TEST);
            ogl33::glDrawArrays(ogl33::GL_TRIANGLES, 0, (quads.len() * 6) as i32);
            ogl33::glEnable(ogl33::GL_DEPTH_TEST);
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
}
impl GUIQuad {
    pub fn new(x: f32, y: f32, w: f32, h: f32, texture: &AtlassedTexture) -> GUIQuad {
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
        }
    }
}

pub enum GUIComponent {
    ImageComponent(f32, f32, f32, f32, AtlassedTexture),
}
impl GUIComponent {
    pub fn add_quads(&self, quads: &mut Vec<GUIQuad>) {
        match self {
            Self::ImageComponent(x, y, w, h, texture) => {
                quads.push(GUIQuad::new(*x, *y, *w, *h, &texture));
            }
        }
    }
}
struct GUI {
    components: Vec<GUIComponent>,
    cursor: (f32, f32),
}
impl GUI {
    pub fn new() -> Self {
        Self {
            components: Vec::new(),
            cursor: (0., 0.),
        }
    }
    pub fn to_quad_list(&self) -> Vec<GUIQuad> {
        let mut quads = Vec::new();
        for component in &self.components {
            component.add_quads(&mut quads);
        }
        quads
    }
}
