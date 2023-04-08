#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Vertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub u: f32,
    pub v: f32,
    pub render_data: u8,
}
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ModelVertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub u: f32,
    pub v: f32,
    pub render_data: u16,
}
unsafe impl bytemuck::Zeroable for Vertex {}
unsafe impl bytemuck::Pod for Vertex {}
unsafe impl bytemuck::Zeroable for ModelVertex {}
unsafe impl bytemuck::Pod for ModelVertex {}
pub type ColorVertex = [f32; 3 + 3];
pub type GuiVertex = [f32; 2 + 2 + 4];
pub type BasicVertex = [f32; 3 + 2];

pub struct Shader {
    shader_program: u32,
}
impl Shader {
    pub fn new(vertex_source: String, fragment_source: String) -> Shader {
        unsafe {
            let vertex_shader = ogl33::glCreateShader(ogl33::GL_VERTEX_SHADER);
            assert_ne!(vertex_shader, 0);
            ogl33::glShaderSource(
                vertex_shader,
                1,
                &(vertex_source.as_bytes().as_ptr().cast()),
                &(vertex_source.len().try_into().unwrap()),
            );
            ogl33::glCompileShader(vertex_shader);
            let mut success = 0;
            ogl33::glGetShaderiv(vertex_shader, ogl33::GL_COMPILE_STATUS, &mut success);
            if success == 0 {
                let mut v: Vec<u8> = Vec::with_capacity(1024);
                let mut log_len = 0_i32;
                ogl33::glGetShaderInfoLog(vertex_shader, 1024, &mut log_len, v.as_mut_ptr().cast());
                v.set_len(log_len.try_into().unwrap());
                panic!("Vertex Compile Error: {}", String::from_utf8_lossy(&v));
            }
            let fragment_shader = ogl33::glCreateShader(ogl33::GL_FRAGMENT_SHADER);
            assert_ne!(fragment_shader, 0);
            ogl33::glShaderSource(
                fragment_shader,
                1,
                &(fragment_source.as_bytes().as_ptr().cast()),
                &(fragment_source.len().try_into().unwrap()),
            );
            ogl33::glCompileShader(fragment_shader);
            let mut success = 0;
            ogl33::glGetShaderiv(fragment_shader, ogl33::GL_COMPILE_STATUS, &mut success);
            if success == 0 {
                let mut v: Vec<u8> = Vec::with_capacity(1024);
                let mut log_len = 0_i32;
                ogl33::glGetShaderInfoLog(
                    fragment_shader,
                    1024,
                    &mut log_len,
                    v.as_mut_ptr().cast(),
                );
                v.set_len(log_len.try_into().unwrap());
                panic!("Fragment Compile Error: {}", String::from_utf8_lossy(&v));
            }
            let shader_program = ogl33::glCreateProgram();
            ogl33::glAttachShader(shader_program, vertex_shader);
            ogl33::glAttachShader(shader_program, fragment_shader);
            ogl33::glLinkProgram(shader_program);
            let mut success = 0;
            ogl33::glGetProgramiv(shader_program, ogl33::GL_LINK_STATUS, &mut success);
            if success == 0 {
                let mut v: Vec<u8> = Vec::with_capacity(1024);
                let mut log_len = 0_i32;
                ogl33::glGetProgramInfoLog(
                    shader_program,
                    1024,
                    &mut log_len,
                    v.as_mut_ptr().cast(),
                );
                v.set_len(log_len.try_into().unwrap());
                panic!("Program Link Error: {}", String::from_utf8_lossy(&v));
            }
            ogl33::glDeleteShader(vertex_shader);
            ogl33::glDeleteShader(fragment_shader);
            return Shader { shader_program };
        }
    }
    pub fn use_program(&self) {
        unsafe {
            ogl33::glUseProgram(self.shader_program);
        }
    }
    pub fn get_uniform_location(&self, name: &str) -> Option<u32> {
        unsafe {
            let location =
                ogl33::glGetUniformLocation(self.shader_program, name.as_ptr() as *const i8);
            if location >= 0 {
                Some(location as u32)
            } else {
                None
            }
        }
    }
    pub fn set_uniform_matrix(&self, uniform_location: u32, matrix: ultraviolet::Mat4) {
        unsafe {
            ogl33::glUniformMatrix4fv(uniform_location as i32, 1, ogl33::GL_FALSE, matrix.as_ptr());
        }
    }
    pub fn set_uniform_matrices(&self, uniform_location: u32, matrices: Vec<ultraviolet::Mat4>) {
        unsafe {
            ogl33::glUniformMatrix4fv(
                uniform_location as i32,
                matrices.len() as i32,
                ogl33::GL_FALSE,
                matrices.as_ptr() as *const f32,
            );
        }
    }
    pub fn set_uniform_float(&self, uniform_location: u32, value: f32) {
        unsafe {
            ogl33::glUniform1f(uniform_location as i32, value);
        }
    }
    pub fn set_uniform_vec3(&self, uniform_location: u32, value: (i32, i32, i32)) {
        unsafe {
            ogl33::glUniform3i(uniform_location as i32, value.0, value.1, value.2);
        }
    }
}
pub struct VertexArray {
    vao_id: u32,
}
impl VertexArray {
    pub fn new() -> Option<VertexArray> {
        let mut vao = 0;
        unsafe { ogl33::glGenVertexArrays(1, &mut vao) };
        if vao != 0 {
            Some(VertexArray { vao_id: vao })
        } else {
            None
        }
    }
    pub fn bind(&self) {
        unsafe { ogl33::glBindVertexArray(self.vao_id) }
    }
    pub fn unbind() {
        unsafe { ogl33::glBindVertexArray(0) }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferType {
    Array = ogl33::GL_ARRAY_BUFFER as isize,
    ElementArray = ogl33::GL_ELEMENT_ARRAY_BUFFER as isize,
}
pub struct Buffer {
    vbo_id: u32,
    vbo_type: BufferType,
}
impl Buffer {
    pub fn new(vbo_type: BufferType) -> Option<Buffer> {
        let mut vbo = 0;
        unsafe {
            ogl33::glGenBuffers(1, &mut vbo);
        }
        if vbo != 0 {
            Some(Buffer {
                vbo_id: vbo,
                vbo_type,
            })
        } else {
            None
        }
    }
    pub fn bind(&self) {
        unsafe { ogl33::glBindBuffer(self.vbo_type as ogl33::GLenum, self.vbo_id) }
    }
    pub fn unbind(&self) {
        unsafe { ogl33::glBindBuffer(self.vbo_type as ogl33::GLenum, 0) }
    }
    pub fn upload_data(&mut self, data: &[u8], usage: ogl33::GLenum) {
        self.bind();
        unsafe {
            ogl33::glBufferData(
                self.vbo_type as ogl33::GLenum,
                data.len().try_into().unwrap(),
                data.as_ptr().cast(),
                usage,
            );
        }
    }
}
impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            ogl33::glDeleteBuffers(1, &self.vbo_id);
        }
    }
}
pub struct Texture {
    tex_id: u32,
}
impl Texture {
    /*pub fn new_from_file(mut image: std::fs::File) -> Option<Texture> {
        Texture::new({
            let mut bytes = vec![];
            std::io::Read::read_to_end(&mut image, &mut bytes).unwrap();
            bytes
        });
        let bitmap = { imagine::png::parse_png_rgba8(&bitmap).unwrap().bitmap };
    }*/
    pub fn new(bitmap: Vec<u8>, width: u32, height: u32) -> Option<Texture> {
        let mut tex_id = 0;
        unsafe {
            ogl33::glGenTextures(1, &mut tex_id);
        }
        if tex_id != 0 {
            unsafe {
                ogl33::glBindTexture(ogl33::GL_TEXTURE_2D, tex_id);
                ogl33::glTexParameteri(
                    ogl33::GL_TEXTURE_2D,
                    ogl33::GL_TEXTURE_WRAP_S,
                    ogl33::GL_REPEAT as ogl33::GLint,
                );
                ogl33::glTexParameteri(
                    ogl33::GL_TEXTURE_2D,
                    ogl33::GL_TEXTURE_WRAP_T,
                    ogl33::GL_REPEAT as ogl33::GLint,
                );
                ogl33::glTexParameteri(
                    ogl33::GL_TEXTURE_2D,
                    ogl33::GL_TEXTURE_MIN_FILTER,
                    ogl33::GL_LINEAR as ogl33::GLint,
                );
                ogl33::glTexParameteri(
                    ogl33::GL_TEXTURE_2D,
                    ogl33::GL_TEXTURE_MAG_FILTER,
                    ogl33::GL_NEAREST as ogl33::GLint,
                );
                println!("width: {} height: {}", width, height);
                ogl33::glTexImage2D(
                    ogl33::GL_TEXTURE_2D,
                    0,
                    ogl33::GL_RGBA as ogl33::GLint,
                    (width as i32).into(),
                    (height as i32).into(),
                    0,
                    ogl33::GL_RGBA,
                    ogl33::GL_UNSIGNED_BYTE,
                    bitmap.as_ptr().cast(),
                );
                ogl33::glGenerateMipmap(ogl33::GL_TEXTURE_2D);
            }
            Some(Texture { tex_id })
        } else {
            None
        }
    }
    pub fn bind(&self) {
        unsafe {
            ogl33::glBindTexture(ogl33::GL_TEXTURE_2D, self.tex_id);
        }
    }
}
