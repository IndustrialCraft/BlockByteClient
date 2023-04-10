#version 330 core
in float frag_tex;
in float frag_time;
out vec4 final_color;
void main() {
  float darkness = 1-abs(frag_time-1);
  final_color = vec4(0.53*(1-darkness), 0.81*(1-darkness), 0.92*(1-darkness), 1);
}