#![recursion_limit="512"]

use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, WebGlRenderingContext as GL};
use yew::services::render::RenderTask;
use yew::services::{RenderService, ConsoleService};
use yew::services::resize::WindowDimensions;
use yew::{html, Component, ComponentLink, Html, NodeRef, ShouldRender};
use yew::events::InputData;
use glam::*;

pub enum SimType
{
    Jacobi,
    GaussSeidel,
}

pub enum Msg {
    Render(f64),
    ResetClicked,
    SimTypeClicked(SimType),
    NumIterationsChanged(InputData),
    StiffnessChanged(InputData),
    WarmStartChanged,
}

pub struct Constraint
{
    p0 : usize,
    p1 : usize,
    length: f32,
    lambda : f32,
    normal: Vec3,
    c_zero : f32,
    c_one : f32,
    lambda_zero : f32,
    lambda_one : f32,
}

impl Constraint {
    fn new(p0 : usize, p1 : usize, positions : &[Vec3]) -> Constraint
    {
        Constraint {
            p0 : p0,
            p1 : p1,
            length : (positions[p0] - positions[p1]).length(),
            lambda : 0.0f32,
            normal: (positions[p0] - positions[p1]).normalize(),
            c_zero : 0.0f32,
            c_one : 0.0f32,
            lambda_zero : 0.0f32,
            lambda_one : 0.0f32,
        }
    }
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
    total_impulse : Vec<Vec3>,
    is_fixed: Vec<bool>,
    constraints : Vec<Constraint>,
    prev_timestamp : f64,
    target_dt: f32,
    num_iterations : i32,
    do_jacobi : bool,
    do_reset: bool,
    stiffness : f32,
    warm_start : bool
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
            total_impulse : vec![],
            is_fixed : vec![],
            constraints : vec![],
            num_particles : 0,
            num_constraints : 0, 
            prev_timestamp : 0.0f64,
            target_dt : 1.0 / 60.0,
            num_iterations : 1,
            do_jacobi : false,
            do_reset: true,
            stiffness : 1e10f32,
            warm_start : true,
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
            Msg::WarmStartChanged =>
            {
                self.warm_start = !self.warm_start;
                true
            }
            Msg::StiffnessChanged(e) =>
            {
                match e.value.parse::<f32>()
                {
                    Ok(f) =>
                    {
                        self.stiffness = f;
                    }
                    Err(_) => {}
                }
                true
            }
            Msg::NumIterationsChanged(e) =>
            {
                self.num_iterations = e.value.parse().unwrap();
                true
            }
            Msg::SimTypeClicked(t)=> {
                match t {
                    SimType::Jacobi => {
                        self.do_jacobi = true;
                    }
                    SimType::GaussSeidel => {
                        self.do_jacobi = false;
                    }
                }
                false
            }
            Msg::ResetClicked => {
                self.do_reset = true;
                false
            }
            Msg::Render(timestamp) => {

                let do_reset = self.do_reset;

                if do_reset
                {
                    self.do_reset = false;
                    self.prev_timestamp = timestamp;

                    self.current_positions.clear();
                    self.previous_positions.clear();
                    self.total_impulse.clear();
                    self.is_fixed.clear();
                    self.constraints.clear();

                    for i in 0..self.num_particles_x
                    {
                        for j in 0..self.num_particles_y
                        {
                            let xpos = i as f32 / self.num_particles_x as f32 - 0.5f32;
                            let ypos = j as f32 / self.num_particles_y as f32 - 0.5f32;
                            self.current_positions.push(vec3(xpos, -ypos, 0.0f32));
                            self.total_impulse.push(vec3(0.0, 0.0, 0.0));

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
                            self.constraints.push(Constraint::new(p0, p1, &self.current_positions));
                        }
                    }

                    for i in 0..self.num_particles_x -1
                    {
                        for j in 0..self.num_particles_y
                        {
                            let p0 = (i*self.num_particles_y + j) as usize;
                            let p1 = ((i+1)*self.num_particles_y + j) as usize;
                            self.constraints.push(Constraint::new(p0, p1, &self.current_positions));

                        }
                    }

                    for i in 0..self.num_particles_x -1
                    {
                        for j in 0..self.num_particles_y - 1
                        {
                            let p0 = (i*self.num_particles_y + j) as usize;
                            let p1 = ((i+1)*self.num_particles_y + j + 1) as usize;
                            self.constraints.push(Constraint::new(p0, p1, &self.current_positions));

                            let p0 = ((i+1)*self.num_particles_y + j) as usize;
                            let p1 = (i*self.num_particles_y + j + 1) as usize;
                            self.constraints.push(Constraint::new(p0, p1, &self.current_positions));
                        }
                    }

                    self.num_particles = self.current_positions.len();
                    self.num_constraints = self.constraints.len();
                }

                let delta_time = (timestamp - self.prev_timestamp) as f32 / 1000.0;

                if delta_time >= self.target_dt
                {
                    //ConsoleService::log(&format!("delta_time: {}", delta_time));
                    self.prev_timestamp = timestamp;

                    let gravity = vec3(0.0f32, -9.8f32, 0.0f32) * 0.1;

                    for i in 0..self.num_particles
                    {
                        let mut p = self.current_positions[i];
                        let p0 = p;
                        let pm1 = self.previous_positions[i];

                        let is_fixed = self.is_fixed[i];

                        if !is_fixed {
                            let mut d = p-pm1;
                            d = d + gravity*self.target_dt;
                            p = p + d; 
                        }

                        self.current_positions[i] = p;
                        self.previous_positions[i] = p0;
                    }

                    

                    for i in 0..self.num_constraints
                    {
                        //self.constraints[i].lambda = 0.0f32;
                    }

                    let aTilde = 1.0f32 / (self.stiffness * self.target_dt * self.target_dt);
                    let mut workspace = vec![vec4(0.0,0.0,0.0,0.0); self.num_particles];
                    
                    for iteration in 0..self.num_iterations
                    {
                        for constraint_index in 0..self.num_constraints
                        {
                            let mut i = constraint_index;

                            if !self.do_jacobi && iteration % 2 == 1{
                                i = self.num_constraints - i - 1;
                            }
                            let mut c = &mut self.constraints[i];
    
                            let p0InvMass = if self.is_fixed[c.p0] {0.0f32} else {1.0f32};
                            let p1InvMass = if self.is_fixed[c.p1] {0.0f32} else {1.0f32};
                            let totalInvMass = p0InvMass + p1InvMass;
                            let p0RelMass = p0InvMass/totalInvMass;
                            let p1RelMass = p1InvMass/totalInvMass;

                            let mut p0 = self.current_positions[c.p0];
                            let mut p1 = self.current_positions[c.p1];
                            

                            let len = (p0-p1).length();
                            let normal = (p0-p1)/len;
    
                            let mut residual = len - c.length;

                            let mut deltaLambda = -(residual + aTilde*c.lambda) / (totalInvMass + aTilde);
                            if iteration == 0 && self.warm_start{
                                deltaLambda += 0.5f32*c.lambda;
                                c.lambda = 0.0f32;
                            }
                            else if iteration == 0
                            {
                                c.lambda = 0.0f32;
                            }
                            c.lambda += deltaLambda;

                            if constraint_index == 1
                            {
                                ConsoleService::log(&format!("deltaLambda: {}", deltaLambda));
                            }

                            let p0Correction = deltaLambda * p0RelMass * normal;
                            let p1Correction = -deltaLambda * p1RelMass * normal;
    
                            if self.do_jacobi
                            {
                                workspace[c.p0] += vec4(p0Correction.x, p0Correction.y, p0Correction.z, 1.0);
                                workspace[c.p1] += vec4(p1Correction.x, p1Correction.y, p1Correction.z, 1.0);
                            }
                            else
                            {
                                p0 += p0Correction;
                                p1 += p1Correction;
    
                                self.current_positions[c.p0] = p0;
                                self.current_positions[c.p1] = p1;
                            }
                        }

                        if self.do_jacobi {
                            for i in 0..self.num_particles {
                                let impulse = workspace[i];
                                self.current_positions[i] += vec3(impulse.x, impulse.y, impulse.z) / impulse.w;
                                workspace[i] = vec4(0.0, 0.0, 0.0, 0.0);
                            }
                        }

                        for i in 0..self.num_constraints {
                            let mut c = &mut self.constraints[i];

                            let p0 = self.current_positions[c.p0];
                            let p1 = self.current_positions[c.p1];

                            let len = (p0 - p1).length();
                            c.c_one = len - c.length;

                            c.lambda_zero = c.lambda_one;
                            c.lambda_one = c.lambda;
                        }
                    }
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
            <div id="container" style="display:flex">
                <canvas ref=self.node_ref.clone() width={self.width} height={self.height} style="position: absolute"/>
                <div id="overlay" style="position: absolute; display:flex; width:20vw; flex-direction:column"> 
                    <div id="sim_type_selector" style="background-color:#96DEEB; border-radius:5px; margin-top:10px; margin-left:10px;
                    padding: 2px;
                    padding-right: 4px;">
                        <form style="padding-left:10px;" action="/action_page.php">
                            <label for="jacobi">{"Jacobi"}</label>
                            <input type="radio" id="jacobi" name="sim_type" value="Jacobi" checked =self.do_jacobi onclick={self.link.callback(|_| Msg::SimTypeClicked(SimType::Jacobi))}/>
                            <label for="gs">{"Gauss-Seidel"}</label>
                            <input type="radio" id="gs" name="sim_type" value="Gauss-Seidel" checked=!self.do_jacobi onclick={self.link.callback(|_| Msg::SimTypeClicked(SimType::GaussSeidel))}/><br/>
                            <label for="iterations">{&format!("Iterations: {}", self.num_iterations)}</label>
                            <input type="range" id="iterations" min="1" max="200" value={self.num_iterations} oninput={self.link.callback(|e| Msg::NumIterationsChanged(e))}/><br/>
                            <label for="stiffness">{"Stiffness"}</label>
                            <input type="number" style="background-color:#fff5fc; margin-left:10px; border:none" id="stiffness" value={self.stiffness} oninput={self.link.callback(|e| Msg::StiffnessChanged(e))}/><br/>
                            <label for="warm_start">{"Warm Start"}</label>
                            <input type="checkbox" id="warm_start" checked =self.warm_start onclick={self.link.callback(|_| Msg::WarmStartChanged)}/><br/>
                        </form>
                        <button class="button" style="background-color:#5756EB" onclick={self.link.callback(|_| Msg::ResetClicked)}>{"Reset"}</button>
                    </div>
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

        gl.uniform3f(color_uniform.as_ref(), lcolor[0], lcolor[1], lcolor[2]);

        gl.draw_elements_with_i32(GL::LINES, line_count, GL::UNSIGNED_INT, 0);

        gl.uniform3f(color_uniform.as_ref(), vcolor[0], vcolor[1], vcolor[2]);

        gl.draw_arrays(GL::POINTS, 0, particle_count);

        let render_frame = self.link.callback(Msg::Render);
        let handle = RenderService::request_animation_frame(render_frame);

        // A reference to the new handle must be retained for the next render to run.
        self.render_loop = Some(handle);
    }
}

fn main() {
    yew::start_app::<Model>();
}
