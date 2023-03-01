#version 460 core

uniform sampler2D frag_texture;
in vec2 frag_tex;
out vec4 final_color;
void main() {
  final_color = texture(frag_texture, frag_tex);
}