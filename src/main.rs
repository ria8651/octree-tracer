use cgmath::*;
use std::time::Instant;
use wgpu::util::DeviceExt;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

mod adaptive;
mod app;
mod compute;
mod octree;
mod cpu_octree;
mod render;
use adaptive::*;
use app::*;
use compute::*;
use octree::*;
use cpu_octree::*;
use render::*;

fn main() {
    // Defualt file path that only works on the terminal
    let path = "files/dragon.rsvo";
    let octree_depth = 12;

    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let mut app = pollster::block_on(App::new(&window, path.to_string(), octree_depth));

    let now = Instant::now();
    event_loop.run(move |event, _, control_flow| {
        app.render.egui_platform.handle_event(&event);
        app.input(&window, &event);
        match event {
            Event::RedrawRequested(_) => {
                match app.render.render(&window) {
                    Ok(_) => {}
                    // Reconfigure the surface if lost
                    Err(wgpu::SurfaceError::Lost) => app.render.resize(app.render.size),
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                    Err(e) => eprintln!("{:?}", e),
                }
                app.update(now.elapsed().as_secs_f64());
            }
            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually
                // request it.
                window.request_redraw();
            }
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => {
                match event {
                    WindowEvent::Resized(physical_size) => {
                        app.render.resize(*physical_size);
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        // new_inner_size is &&mut so we have to dereference it twice
                        app.render.resize(**new_inner_size);
                    }
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Q),
                                ..
                            },
                        ..
                    } => *control_flow = ControlFlow::Exit,
                    _ => {}
                }
            }
            _ => {}
        }
    });
}

pub struct Input {
    forward: bool,
    backward: bool,
    right: bool,
    left: bool,
    up: bool,
    down: bool,
    mouse_delta: Vector2<f32>,
}

impl Input {
    fn new() -> Self {
        Self {
            forward: false,
            backward: false,
            right: false,
            left: false,
            up: false,
            down: false,
            mouse_delta: Vector2::zero(),
        }
    }
}

pub struct Settings {
    octree_depth: u32,
    fov: f32,
    sensitivity: f32,
    error_string: String,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Zeroable)]
pub struct Uniforms {
    camera: [[f32; 4]; 4],
    camera_inverse: [[f32; 4]; 4],
    dimensions: [f32; 4],
    sun_dir: [f32; 4],
    max_depth: u32,
    pause_adaptive: bool,
    show_steps: bool,
    show_hits: bool,
    shadows: bool,
    misc_value: f32,
    misc_bool: bool,
    junk: [u32; 8],
}

// For bool
unsafe impl bytemuck::Pod for Uniforms {}

impl Uniforms {
    fn new(max_depth: u32) -> Self {
        Self {
            camera: [[0.0; 4]; 4],
            camera_inverse: [[0.0; 4]; 4],
            dimensions: [0.0, 0.0, 0.0, 0.0],
            sun_dir: [-1.7, -1.0, 0.8, 0.0],
            max_depth,
            pause_adaptive: false,
            show_steps: false,
            show_hits: false,
            shadows: true,
            misc_value: 0.0,
            misc_bool: false,
            junk: [0; 8],
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Zeroable, bytemuck::Pod)]
pub struct CUniforms {
    node_length: u32,
    max_depth: u32,
}

impl CUniforms {
    fn new(max_depth: u32) -> Self {
        Self {
            node_length: 0,
            max_depth,
        }
    }
}

pub struct Character {
    pos: Point3<f32>,
    look: Vector3<f32>,
    cursour_grabbed: bool,
    speed: f32,
}

impl Character {
    fn new() -> Self {
        Self {
            pos: Point3::new(0.1, 0.2, -1.5),
            look: -Vector3::new(0.0, 0.0, -1.5),
            cursour_grabbed: true,
            speed: -5.0,
        }
    }
}

fn create_proj_matrix(fov: f32, aspect: f32) -> Matrix4<f32> {
    let s = 1.0 / ((fov / 2.0) * (std::f32::consts::PI / 180.0)).tan();
    Matrix4::new(
        aspect * s,
        0.0,
        0.0,
        0.0,
        //
        0.0,
        s,
        0.0,
        0.0,
        //
        0.0,
        0.0,
        -1.0,
        0.0,
        //
        0.0,
        0.0,
        0.0,
        1.0,
    )
}
