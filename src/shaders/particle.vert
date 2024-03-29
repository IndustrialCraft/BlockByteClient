#version 330
layout ( location = 0 ) in vec2 vertex_position;
layout ( location = 1 ) in vec3 position;
layout ( location = 2 ) in vec4 color;
layout ( location = 3 ) in float size;
uniform mat4 projection;
uniform mat4 view;

out vec4 col;
void main()
{
   vec4 position_viewspace = view * vec4( position.xyz , 1 );
   position_viewspace.xy += size * (vertex_position.xy - vec2(0.5));
   gl_Position = projection * position_viewspace;
   col = color;
}