#version 330 core
uniform mat4 model;
uniform mat4 projection_view;
uniform float time;

layout (location = 0) in vec3 pos;
layout (location = 1) in vec2 tex;
layout (location = 2) in int render_data;
out vec2 frag_tex;
void main() {
  vec3 position = pos;
  if(render_data == 1)
    position.y += sin(time)*0.1;
  gl_Position = projection_view * model * vec4(position, 1.0);
  frag_tex = tex;
}