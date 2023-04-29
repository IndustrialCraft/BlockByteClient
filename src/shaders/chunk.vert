#version 330 core
uniform mat4 model;
uniform mat4 projection_view;
uniform float time;

layout (location = 0) in vec3 pos;
layout (location = 1) in vec2 tex;
layout (location = 2) in int render_data;
layout (location = 3) in int light;

#define PI 3.14159265359

out vec2 frag_tex;
flat out int frag_light;
void main() {
  vec3 position = pos;
  if(render_data == 1)
    position.y += sin(time+(position.x/16*2*PI)+(position.z/16*2*PI*3))*0.1 - 0.1;
  if(render_data == 2){
    position.z += cos(time+ceil(position.x)+50)*0.2*mod(position.y,1.);        
    position.x += sin(time+ceil(position.x))*0.2*mod(position.y,1.);
  }
  if(render_data == 4){
    position.z += cos(time*2+position.y)*0.03;        
    position.x += sin(time*2+position.y+1)*0.03;
    position.y += cos(time*2+position.y+2)*0.03;    
  }
  gl_Position = projection_view * model * vec4(position, 1.0);
  frag_tex = tex;
  frag_light = light;
}