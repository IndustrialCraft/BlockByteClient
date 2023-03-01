#version 330 core

in vec3 frag_col;
out vec4 final_color;
void main() {
  final_color = vec4(frag_col.xyz, 1.0);
}