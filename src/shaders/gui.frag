#version 330 core
uniform sampler2D frag_texture;
in vec2 frag_tex;
in vec4 frag_col;
out vec4 final_color;
void main() {
  if(frag_tex.x == 0 && frag_tex.y == 0){
    final_color = frag_col;
  } else {
    final_color = texture(frag_texture, frag_tex)*frag_col;
  }
}