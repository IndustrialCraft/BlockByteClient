#version 330 core
uniform mat4 model;
uniform mat4 projection_view;

layout (location = 0) in vec3 pos;
layout (location = 1) in vec3 col;
out vec3 frag_col;
void main() {
  gl_Position = projection_view * model * vec4(pos, 1.0);
  frag_col = col;
}