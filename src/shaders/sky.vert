#version 330 core
uniform mat4 projection_view;
uniform float daytime;

layout (location = 0) in vec3 pos;
layout (location = 1) in float tex;
out float frag_tex;
out float frag_time;
void main() {
  vec4 pos = projection_view * vec4(pos, 1.0);
  frag_tex = tex;
  frag_time = daytime;
  gl_Position = pos.xyww;
}
