#version 460 core
uniform mat4 model;
uniform mat4 projection_view;

const int MAX_BONES = 100;
uniform mat4 bones[MAX_BONES];

layout (location = 0) in vec3 pos;
layout (location = 1) in vec2 uv;
layout (location = 2) in int render_data;
out vec2 frag_tex;
void main() {
  gl_Position = projection_view * model * bones[render_data] * vec4(pos, 1.0);
  frag_tex = uv;
}