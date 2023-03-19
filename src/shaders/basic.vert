#version 330 core
uniform mat4 projection_view;

layout (location = 0) in vec3 pos;
layout (location = 1) in vec2 tex;
out vec2 frag_tex;
void main() {
  gl_Position = projection_view * vec4(pos, 1.0);
  frag_tex = tex;
}
