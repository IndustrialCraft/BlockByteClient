#version 330 core

layout (location = 0) in vec2 pos;
layout (location = 1) in vec2 tex;
layout (location = 2) in vec4 col;
out vec2 frag_tex;
out vec4 frag_col;
void main() {
  gl_Position = vec4(pos, 0, 1);
  frag_tex = tex;
  frag_col = col;
}