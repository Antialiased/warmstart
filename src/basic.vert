//#version 300 es
precision mediump float;

attribute vec2 a_position;
uniform float u_aspect_ratio;

void main() {
    gl_PointSize = 5.0;
    gl_Position = vec4(a_position.x / u_aspect_ratio, a_position.y, 0.0, 1.0);
}
