#version 330 core
uniform sampler2D frag_texture;
in vec2 frag_tex;
flat in int frag_light;
out vec4 final_color;
void main() {
  final_color = texture(frag_texture, frag_tex);
  if(final_color[3] == 0)
    discard;
  
  final_color.x *= max(float(frag_light&15), (frag_light>>12)&15)/16.0f;
  final_color.y *= max(float((frag_light>>4)&15), ((frag_light>>12)&15))/16.0f;
  final_color.z *= max(float((frag_light>>8)&15), ((frag_light>>12)&15))/16.0f;
}