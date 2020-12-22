use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, WebGlRenderingContext as GL};
use yew::services::render::RenderTask;
use yew::services::{RenderService, ConsoleService};
use yew::services::resize::WindowDimensions;
use yew::{html, Component, ComponentLink, Html, NodeRef, ShouldRender};
use glam::*;
use std::hash::{Hash, Hasher};

pub enum Msg {
    Render(f64),
}

pub struct Constraint
{
    p0 : usize,
    p1 : usize,
    length: f32,
}


pub struct Model {
    canvas: Option<HtmlCanvasElement>,
    gl: Option<GL>,
    link: ComponentLink<Self>,
    node_ref: NodeRef,
    render_loop: Option<RenderTask>,
    width : i32,
    height : i32,
    num_particles_x : i32,
    num_particles_y : i32,
    num_particles : usize,
    num_constraints : usize,
    current_positions : Vec<Vec3>,
    previous_positions : Vec<Vec3>,
    is_fixed: Vec<bool>,
    constraints : Vec<Constraint>,
    prev_timestamp : f64,
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(_props: Self::Properties, link: ComponentLink<Self>) -> Self {
        Self {
            canvas: None,
            gl: None,
            link,
            node_ref: NodeRef::default(),
            render_loop: None,
            width : 100,
            height : 100,
            num_particles_x : 10,
            num_particles_y : 10,
            current_positions: vec![],
            previous_positions: vec![],
            is_fixed : vec![],
            constraints : vec![],
            num_particles : 0,
            num_constraints : 0, 
            prev_timestamp : 0.0f64,
        }
    }

    fn rendered(&mut self, first_render: bool) {
        // Once rendered, store references for the canvas and GL context. These can be used for
        // resizing the rendering area when the window or canvas element are resized, as well as
        // for making GL calls.

        let canvas = self.node_ref.cast::<HtmlCanvasElement>().unwrap();

        let gl: GL = canvas
            .get_context("webgl")
            .unwrap()
            .unwrap()
            .dyn_into()
            .unwrap();

        self.canvas = Some(canvas);
        self.gl = Some(gl);

        // In a more complex use-case, there will be additional WebGL initialization that should be
        // done here, such as enabling or disabling depth testing, depth functions, face
        // culling etc.

        if first_render {
            // The callback to request animation frame is passed a time value which can be used for
            // rendering motion independent of the framerate which may vary.
            let render_frame = self.link.callback(Msg::Render);
            let handle = RenderService::request_animation_frame(render_frame);
            
            // A reference to the handle must be stored, otherwise it is dropped and the render won't
            // occur.
            self.render_loop = Some(handle);

        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::Render(timestamp) => {

                let do_reset = self.current_positions.len() == 0;

                if do_reset
                {
                    self.prev_timestamp = timestamp;

                    self.current_positions.clear();
                    self.previous_positions.clear();
                    self.is_fixed.clear();
                    self.constraints.clear();

                    for i in 0..self.num_particles_x
                    {
                        for j in 0..self.num_particles_y
                        {
                            let xpos = i as f32 / self.num_particles_x as f32 - 0.5f32;
                            let ypos = j as f32 / self.num_particles_y as f32 - 0.5f32;
                            self.current_positions.push(vec3(xpos, -ypos, 0.0f32));

                            self.is_fixed.push(j == 0 && (i == 0 || i == self.num_particles_x-1));
                        }
                    }

                    self.previous_positions = self.current_positions.clone();

                    for i in 0..self.num_particles_x
                    {
                        for j in 0..self.num_particles_y-1
                        {
                            let p0 = (i*self.num_particles_y + j) as usize;
                            let p1 = (i*self.num_particles_y + j + 1) as usize;

                            let length = (self.current_positions[p0] - self.current_positions[p1]).length();

                            self.constraints.push(Constraint {p0, p1, length});
                        }
                    }

                    for i in 0..self.num_particles_x -1
                    {
                        for j in 0..self.num_particles_y
                        {
                            let p0 = (i*self.num_particles_y + j) as usize;
                            let p1 = ((i+1)*self.num_particles_y + j) as usize;

                            let length = (self.current_positions[p0] - self.current_positions[p1]).length();

                            self.constraints.push(Constraint {p0, p1, length});
                        }
                    }

                    self.num_particles = self.current_positions.len();
                    self.num_constraints = self.constraints.len();
                }

                let delta_time = (timestamp - self.prev_timestamp) as f32 / 1000.0;
                ConsoleService::log(&format!("delta_time: {}", delta_time));
                self.prev_timestamp = timestamp;

                let gravity = vec3(0.0f32, -9.8f32*0.01, 0.0f32);

                for i in 0..self.num_particles
                {
                    let mut p = self.current_positions[i];
                    let p0 = p;
                    let pm1 = self.previous_positions[i];

                    let is_fixed = self.is_fixed[i];

                    if !is_fixed {
                        let mut v = (p-pm1) * (1.0f32 / delta_time);
                        v = v + gravity*delta_time;
                        p = p + v*delta_time / 1000.0; 
                    }

                    self.current_positions[i] = p;
                    self.previous_positions[i] = p0;
                }

                // Render functions are likely to get quite large, so it is good practice to split
                // it into it's own function rather than keeping it inline in the update match
                // case. This also allows for updating other UI elements that may be rendered in
                // the DOM like a framerate counter, or other overlaid textual elements.
                self.render_gl(timestamp);

                let window = web_sys::window().unwrap();
                let dimensions = WindowDimensions::get_dimensions(&window);
                let width = dimensions.width;
                let height = dimensions.height;

                let should_render = !(width == self.width && height == self.height);

                self.width = width;
                self.height = height;
                should_render
            }
        }
    }

    fn view(&self) -> Html {
        html! {
            <div id="container">
                <canvas ref=self.node_ref.clone() width={self.width} height={self.height} style="position: absolute"/>
                <div id="overlay" style="position: absolute">
                    <button class="button button1">{"2px"}</button>
                    <button class="button button2">{"4px"}</button>
                    <button class="button button3">{"8px"}</button>
                    <button class="button button4">{"12px"}</button>
                    <button class="button button5">{"50%"}</button>
                </div>
            </div>
        }
    }

    fn change(&mut self, _props: Self::Properties) -> ShouldRender {
        false
    }
}

impl Model {
    fn render_gl(&mut self, timestamp: f64) {
        let gl = self.gl.as_ref().expect("GL Context not initialized!");
        let _ext = gl.get_extension("OES_element_index_uint");

        let vert_code = include_str!("./basic.vert");
        let frag_code = include_str!("./basic.frag");

        let particle_count = self.num_particles as i32;
        let line_count = self.num_constraints as i32 * 2;

        gl.viewport(0, 0, self.width, self.height);

        let vertex_buffer = gl.create_buffer().unwrap();

        let mut vertex_positions : Vec<f32> = vec![];
        
        self.current_positions.iter().for_each(|v| {vertex_positions.push(v.x); vertex_positions.push(v.y)});

        let verts = js_sys::Float32Array::from(vertex_positions.as_slice());

        let mut edges : Vec<i32> = vec![];
        self.constraints.iter().for_each(|c| {edges.push(c.p0 as i32); edges.push(c.p1 as i32)});

        let index_buffer = gl.create_buffer().unwrap();
        let indices = js_sys::Int32Array::from(edges.as_slice());


        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&vertex_buffer));
        gl.buffer_data_with_array_buffer_view(GL::ARRAY_BUFFER, &verts, GL::STATIC_DRAW);
        
        gl.bind_buffer(GL::ELEMENT_ARRAY_BUFFER, Some(&index_buffer));
        gl.buffer_data_with_array_buffer_view(GL::ELEMENT_ARRAY_BUFFER, &indices, GL::STATIC_DRAW);


        let vert_shader = gl.create_shader(GL::VERTEX_SHADER).unwrap();
        gl.shader_source(&vert_shader, &vert_code);
        gl.compile_shader(&vert_shader);

        let frag_shader = gl.create_shader(GL::FRAGMENT_SHADER).unwrap();
        gl.shader_source(&frag_shader, &frag_code);
        gl.compile_shader(&frag_shader);

        let shader_program = gl.create_program().unwrap();
        gl.attach_shader(&shader_program, &vert_shader);
        gl.attach_shader(&shader_program, &frag_shader);
        gl.link_program(&shader_program);

        gl.use_program(Some(&shader_program));

        // Attach the position vector as an attribute for the GL context.
        let position = gl.get_attrib_location(&shader_program, "a_position") as u32;
        gl.vertex_attrib_pointer_with_i32(position, 2, GL::FLOAT, false, 0, 0);
        gl.enable_vertex_attrib_array(position);

        // Attach the time as a uniform for the GL context.
        let time = gl.get_uniform_location(&shader_program, "u_time");
        gl.uniform1f(time.as_ref(), timestamp as f32);

        let aspect_ratio = self.width as f32 / self.height as f32;
        let aspect_ratio_uniform = gl.get_uniform_location(&shader_program, "u_aspect_ratio");
        gl.uniform1f(aspect_ratio_uniform.as_ref(), aspect_ratio);

        let vcolor = vec![1.0f32, 0.0f32, 0.0f32];
        let lcolor = vec![0.0f32, 0.0f32, 0.0f32];

        let color_uniform = gl.get_uniform_location(&shader_program, "u_color");

        gl.uniform3f(color_uniform.as_ref(), vcolor[0], vcolor[1], vcolor[2]);

        gl.draw_arrays(GL::POINTS, 0, particle_count);

        gl.uniform3f(color_uniform.as_ref(), lcolor[0], lcolor[1], lcolor[2]);

        gl.draw_elements_with_i32(GL::LINES, line_count, GL::UNSIGNED_INT, 0);

        let render_frame = self.link.callback(Msg::Render);
        let handle = RenderService::request_animation_frame(render_frame);

        // A reference to the new handle must be retained for the next render to run.
        self.render_loop = Some(handle);
    }
}

fn main() {
    yew::start_app::<Model>();
}