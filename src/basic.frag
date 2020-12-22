//#version 300 es
precision mediump float;

uniform float u_time;
uniform vec3 u_color;

void main() {
    gl_FragColor = vec4(u_color, 1.0);
}
