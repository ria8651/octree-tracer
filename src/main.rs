use cgmath::prelude::*;
use cgmath::{perspective, Deg, Euler, Matrix4, Point3, Quaternion, Rad, Vector2, Vector3};
use std::time::Instant;
use wgpu::util::DeviceExt;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

fn main() {
    // Defualt file path that only works on the terminal
    let path = "files/dragon.rsvo";
    let svo_depth = 12;

    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let mut state = pollster::block_on(State::new(&window, path.to_string(), svo_depth));

    let now = Instant::now();
    event_loop.run(move |event, _, control_flow| {
        state.egui_platform.handle_event(&event);
        state.input(&window, &event);
        match event {
            Event::RedrawRequested(_) => {
                match state.render(&window) {
                    Ok(_) => {}
                    // Reconfigure the surface if lost
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                    Err(e) => eprintln!("{:?}", e),
                }
                state.update(&window, now.elapsed().as_secs_f64());
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
                        state.resize(*physical_size);
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        // new_inner_size is &&mut so we have to dereference it twice
                        state.resize(**new_inner_size);
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

struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
    uniforms: Uniforms,
    uniform_buffer: wgpu::Buffer,
    storage_buffer: wgpu::Buffer,
    // main_bind_group_layout: wgpu::BindGroupLayout,
    main_bind_group: wgpu::BindGroup,
    input: Input,
    character: Character,
    previous_frame_time: Option<f64>,
    egui_platform: egui_winit_platform::Platform,
    egui_rpass: egui_wgpu_backend::RenderPass,
    settings: Settings,
}

impl State {
    // Creating some of the wgpu types requires async code
    async fn new(window: &Window, svo_path: String, svo_depth: u32) -> Self {
        let error_string = "".to_string();

        let settings = Settings {
            svo_depth,
            fov: 90.0,
            sensitivity: 0.006,
            error_string,
        };

        window.set_cursor_grab(true).unwrap();
        window.set_cursor_visible(false);

        // The instance is a handle to our GPU
        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits {
                        max_storage_buffer_binding_size: 1024000000,
                        ..Default::default()
                    },
                    label: None,
                },
                None, // Trace path
            )
            .await
            .unwrap();

        // println!("Info: {:?}", device.limits().max_storage_buffer_binding_size);

        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_preferred_format(&adapter).unwrap(),
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(
                (concat!(include_str!("common.wgsl"), include_str!("shader.wgsl"))).into(),
            ),
        });

        // #region Buffers
        let uniforms = Uniforms::new();
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let mut defualt_octree = CpuOctree::new(0);
        defualt_octree.put_in_voxel(Vector3::new(1.0, 1.0, 1.0), Leaf::new(1), 3);
        defualt_octree.put_in_voxel(Vector3::new(0.0, 0.0, 0.0), Leaf::new(1), 3);
        defualt_octree.put_in_voxel(Vector3::new(-1.0, -1.0, -1.0), Leaf::new(1), 3);

        let mut svo = match load_file(svo_path, svo_depth) {
            Ok(svo) => svo,
            Err(_) => defualt_octree,
        };

        // So we can load a bigger file later
        svo.nodes
            .extend(std::iter::repeat(0).take(256000000 - svo.nodes.len()));

        let storage_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("DF Buffer"),
            contents: bytemuck::cast_slice(&svo.nodes),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });

        let main_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
                label: Some("main_bind_group_layout"),
            });

        let main_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &main_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: storage_buffer.as_entire_binding(),
                },
            ],
            label: Some("uniform_bind_group"),
        });
        // #endregion

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&main_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                }],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        let input = Input::new();
        let character = Character::new();

        // egui
        let size = window.inner_size();
        let egui_platform =
            egui_winit_platform::Platform::new(egui_winit_platform::PlatformDescriptor {
                physical_width: size.width as u32,
                physical_height: size.height as u32,
                scale_factor: window.scale_factor(),
                font_definitions: egui::FontDefinitions::default(),
                style: Default::default(),
            });

        // We use the egui_wgpu_backend crate as the render backend.
        let egui_rpass = egui_wgpu_backend::RenderPass::new(&device, config.format, 1);

        let previous_frame_time = None;

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            uniforms,
            uniform_buffer,
            storage_buffer,
            // main_bind_group_layout,
            main_bind_group,
            input,
            character,
            previous_frame_time,
            egui_platform,
            egui_rpass,
            settings,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn input(&mut self, window: &Window, event: &Event<()>) {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state,
                            virtual_keycode,
                            ..
                        },
                    ..
                } => match virtual_keycode {
                    Some(VirtualKeyCode::W) => {
                        self.input.forward = *state == ElementState::Pressed;
                    }
                    Some(VirtualKeyCode::S) => {
                        self.input.backward = *state == ElementState::Pressed;
                    }
                    Some(VirtualKeyCode::D) => {
                        self.input.right = *state == ElementState::Pressed;
                    }
                    Some(VirtualKeyCode::A) => {
                        self.input.left = *state == ElementState::Pressed;
                    }
                    Some(VirtualKeyCode::Space) => {
                        self.input.up = *state == ElementState::Pressed;
                    }
                    Some(VirtualKeyCode::LShift) => {
                        self.input.down = *state == ElementState::Pressed;
                    }
                    //
                    Some(VirtualKeyCode::Escape) => {
                        window.set_cursor_grab(false).unwrap();
                        window.set_cursor_visible(true);
                    }
                    _ => {}
                },
                _ => {}
            },
            Event::DeviceEvent { event, .. } => match event {
                DeviceEvent::MouseMotion { delta } => {
                    self.input.mouse_delta = Vector2::new(delta.0 as f32, delta.1 as f32);
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn update(&mut self, window: &Window, time: f64) {
        let input = Vector3::new(
            self.input.right as u32 as f32 - self.input.left as u32 as f32,
            self.input.up as u32 as f32 - self.input.down as u32 as f32,
            self.input.forward as u32 as f32 - self.input.backward as u32 as f32,
        ) * 0.01;

        let forward: Vector3<f32> = self.character.look.normalize();
        let right = forward.cross(Vector3::unit_y()).normalize();
        let up = right.cross(forward);

        self.character.pos += forward * input.z + right * input.x + up * input.y;

        let delta = self.settings.sensitivity * self.input.mouse_delta;
        let rotation = Quaternion::from_axis_angle(right, Rad(-delta.y))
            * Quaternion::from_axis_angle(Vector3::unit_y(), Rad(-delta.x));

        self.input.mouse_delta = Vector2::zero();
        self.character.look = (rotation * self.character.look).normalize();

        let dimensions = [self.size.width as f32, self.size.height as f32];

        let view = Matrix4::<f32>::look_at_rh(
            self.character.pos,
            self.character.pos + self.character.look,
            Vector3::unit_y(),
        );
        let proj = perspective(
            Deg(self.settings.fov),
            dimensions[0] / dimensions[1],
            0.001,
            1.0,
        );
        let camera = proj * view;
        let camera_inverse = camera.invert().unwrap();

        self.uniforms.dimensions = [dimensions[0], dimensions[1], 0.0, 0.0];
        self.uniforms.camera = camera.into();
        self.uniforms.camera_inverse = camera_inverse.into();

        let fps = if let Some(previous_frame_time) = self.previous_frame_time {
            let fps = 1.0 / (time - previous_frame_time);
            self.previous_frame_time = Some(time);
            fps
        } else {
            self.previous_frame_time = Some(time);
            0.0
        };

        egui::Window::new("Info").show(&self.egui_platform.context(), |ui| {
            ui.label(format!("FPS: {:.0}", fps));
            // let mut max_depth = 0;
            // ui.add(egui::Slider::new(&mut max_depth, 1..=16).text("Max depth"));
            if ui.button("Open File").clicked() {
                let path = native_dialog::FileDialog::new()
                    .add_filter("Magica Voxel RSVO File", &["rsvo"])
                    .add_filter("Magica Voxel Vox File", &["vox"])
                    .show_open_single_file()
                    .unwrap();

                match path {
                    Some(path) => match load_file(
                        path.into_os_string().into_string().unwrap(),
                        self.settings.svo_depth,
                    ) {
                        Ok(svo) => {
                            self.queue.write_buffer(
                                &self.storage_buffer,
                                0,
                                bytemuck::cast_slice(&svo.nodes),
                            );
                            self.settings.error_string = "".to_string();
                        }
                        Err(e) => {
                            self.settings.error_string = e;
                        }
                    },
                    None => self.settings.error_string = "No file selected".to_string(),
                }
            }

            ui.add(egui::Slider::new(&mut self.settings.svo_depth, 0..=20).text("SVO depth"));
            ui.add(egui::Slider::new(&mut self.settings.fov, 0.1..=120.0).text("FOV"));
            ui.add(
                egui::Slider::new(&mut self.settings.sensitivity, 0.001..=0.01).text("Sensitivity"),
            );

            ui.horizontal(|ui| {
                ui.label("x: ");
                ui.add(egui::DragValue::new(&mut self.uniforms.sun_dir[0]).speed(0.1));
                ui.label("y: ");
                ui.add(egui::DragValue::new(&mut self.uniforms.sun_dir[1]).speed(0.1));
                ui.label("z: ");
                ui.add(egui::DragValue::new(&mut self.uniforms.sun_dir[2]).speed(0.1));
            });

            ui.checkbox(&mut self.uniforms.show_steps, "Show ray steps");
            ui.checkbox(&mut self.uniforms.shadows, "Shadows");
            ui.add(egui::Slider::new(&mut self.uniforms.misc_value, 0.0..=0.01).text("Misc"));
            ui.checkbox(&mut self.uniforms.misc_bool, "Misc");
            
            if ui.button("Grab cursour").clicked() {
                window.set_cursor_grab(true).unwrap();
                window.set_cursor_visible(false);
            }
        });

        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.uniforms]),
        );

        self.egui_platform.update_time(time);
    }

    fn render(&mut self, window: &Window) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let size = window.inner_size();

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Draw my app
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.main_bind_group, &[]);
            render_pass.draw(0..4, 0..1);
        }

        // Draw the UI frame.
        self.egui_platform.begin_frame();

        // End the UI frame. We could now handle the output and draw the UI with the backend.
        let (_output, paint_commands) = self.egui_platform.end_frame(Some(window));
        let paint_jobs = self.egui_platform.context().tessellate(paint_commands);

        // Upload all resources for the GPU.
        let screen_descriptor = egui_wgpu_backend::ScreenDescriptor {
            physical_width: size.width,
            physical_height: size.height,
            scale_factor: window.scale_factor() as f32,
        };
        self.egui_rpass.update_texture(
            &self.device,
            &self.queue,
            &self.egui_platform.context().font_image(),
        );
        self.egui_rpass
            .update_user_textures(&self.device, &self.queue);
        self.egui_rpass
            .update_buffers(&self.device, &self.queue, &paint_jobs, &screen_descriptor);

        // Record all render passes.
        self.egui_rpass
            .execute(&mut encoder, &view, &paint_jobs, &screen_descriptor, None)
            .unwrap();

        // Submit the command buffer.
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

struct Input {
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

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Zeroable)]
struct Uniforms {
    camera: [[f32; 4]; 4],
    camera_inverse: [[f32; 4]; 4],
    dimensions: [f32; 4],
    sun_dir: [f32; 4],
    show_steps: bool,
    shadows: bool,
    misc_value: f32,
    misc_bool: bool,
    junk: [u32; 8],
}

struct Settings {
    svo_depth: u32,
    fov: f32,
    sensitivity: f32,
    error_string: String,
}

// For bool
unsafe impl bytemuck::Pod for Uniforms {}

impl Uniforms {
    fn new() -> Self {
        Self {
            camera: [[0.0; 4]; 4],
            camera_inverse: [[0.0; 4]; 4],
            dimensions: [0.0, 0.0, 0.0, 0.0],
            sun_dir: [-1.7, -1.0, 0.8, 0.0],
            show_steps: false,
            shadows: true,
            misc_value: 0.0,
            misc_bool: false,
            junk: [0; 8],
        }
    }
}

struct Character {
    pos: Point3<f32>,
    look: Vector3<f32>,
}

impl Character {
    fn new() -> Self {
        Self {
            pos: Point3::new(0.0, 0.0, -1.5),
            look: -Vector3::new(0.0, 0.0, -1.5),
        }
    }
}

// #region Create octree
fn load_file(file: String, svo_depth: u32) -> Result<CpuOctree, String> {
    let path = std::path::Path::new(&file);
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    use std::ffi::OsStr;
    let octree = match path.extension().and_then(OsStr::to_str) {
        Some("rsvo") => load_octree(&data, svo_depth),
        Some("vox") => load_vox(&data),
        _ => Err("Unknown file type".to_string()),
    }?;

    return Ok(octree);
}

// Models from https://github.com/ephtracy/voxel-model/tree/master/svo
fn load_octree(data: &[u8], octree_depth: u32) -> Result<CpuOctree, String> {
    let top_level_start = 16;
    let node_count_start = 20;

    let top_level = data[top_level_start] as usize;

    let data_start = node_count_start + 4 * (top_level + 1);

    let mut node_counts = Vec::new();
    for i in 0..(top_level + 1) {
        let node_count = u32::from_be_bytes([
            data[node_count_start + i * 4 + 3],
            data[node_count_start + i * 4 + 2],
            data[node_count_start + i * 4 + 1],
            data[node_count_start + i * 4],
        ]);

        node_counts.push(node_count);
    }

    let node_end = node_counts[0..octree_depth as usize].iter().sum::<u32>() as usize;

    let mut octree = CpuOctree::new(data[data_start]);

    let mut data_index = 1;
    let mut node_index = 0;
    while node_index < octree.nodes.len() {
        if octree.nodes[node_index] != PALETTE_OFFSET {
            if data_index < node_end {
                let child_mask = data[data_start + data_index];
                octree.subdivide(node_index, child_mask);
            } else {
                octree.nodes[node_index] = Leaf::new(1);
            }

            data_index += 1;
        }

        node_index += 1;
    }

    println!("SVO size: {}", octree.nodes.len());
    Ok(octree)
}

fn load_vox(file: &[u8]) -> Result<CpuOctree, String> {
    let vox_data = dot_vox::load_bytes(file)?;
    let size = vox_data.models[0].size;
    if size.x != size.y || size.x != size.z || size.y != size.z {
        return Err("Voxel model is not a cube!".to_string());
    }

    let size = size.x as i32;

    let depth = (size as f32).log2();
    if depth != depth.floor() {
        return Err("Voxel model size is not a power of 2!".to_string());
    }

    let mut octree = CpuOctree::new(0x00000000);
    for voxel in &vox_data.models[0].voxels {
        // let colour = vox_data.palette[voxel.i as usize].to_le_bytes();
        let mut pos = Vector3::new(
            size as f32 - voxel.x as f32 - 1.0,
            voxel.z as f32,
            voxel.y as f32,
        );
        pos /= size as f32;
        pos = pos * 2.0 - Vector3::new(1.0, 1.0, 1.0);

        octree.put_in_voxel(pos, Leaf::new(1), depth as u32);
    }

    println!("SVO size: {}", octree.nodes.len());
    return Ok(octree);
}

// First palette colour is empty voxel
const PALETTE: [u32; 3] = [0x00000000, 0x0000FF00, 0x000000FF];
const PALETTE_SIZE: u32 = 65536;
const PALETTE_OFFSET: u32 = u32::MAX - PALETTE_SIZE;

// Layout
// 01100101 01100101 01100101 01100101
//  ^---- Node: Pointer to children, Voxel: Palette index
// ^----- 0: Node, 1: Voxel
struct CpuOctree {
    nodes: Vec<u32>,
}

struct Leaf {
    r: u8,
    g: u8,
    b: u8,
}
impl Leaf {
    fn new(palette_index: u32) -> u32 {
        palette_index + PALETTE_OFFSET
    }

    fn unpack(v: u32) -> Option<Leaf> {
        let palette_index = v - PALETTE_OFFSET;
        if palette_index == 0 {
            None
        } else {
            let palette_colour = PALETTE[palette_index as usize];
            Some(Leaf {
                r: (palette_colour >> 16) as u8,
                g: (palette_colour >> 8) as u8,
                b: palette_colour as u8,
            })
        }
    }
}

impl CpuOctree {
    fn new(mask: u8) -> Self {
        let mut nodes = Vec::new();

        // Add 8 new voxels
        for i in 0..8 {
            if mask >> i & 1 != 0 {
                nodes.push(Leaf::new(1));
            } else {
                nodes.push(Leaf::new(0));
            }
        }

        Self { nodes }
    }

    fn subdivide(&mut self, node: usize, mask: u8) {
        if self.nodes[node] < PALETTE_OFFSET {
            panic!("Node already subdivided!");
        }

        // Turn voxel into node
        self.nodes[node] = self.nodes.len() as u32;

        // Add 8 new voxels
        for i in 0..8 {
            if mask >> i & 1 != 0 {
                self.nodes.push(Leaf::new(1));
            } else {
                self.nodes.push(Leaf::new(0));
            }
        }
    }

    fn put_in_voxel(&mut self, pos: Vector3<f32>, value: u32, depth: u32) {
        loop {
            let (node, node_depth) = self.get_node(pos);
            if depth == node_depth {
                self.nodes[node] = value;
                return;
            } else {
                self.subdivide(node, 0x00000000);
            }
        }
    }

    fn get_node(&self, pos: Vector3<f32>) -> (usize, u32) {
        let mut node_index = 0;
        let mut node_pos = Vector3::new(0.0, 0.0, 0.0);
        let mut depth = 0;
        loop {
            depth += 1;

            let p = Vector3::new(
                (pos.x >= node_pos.x) as usize,
                (pos.y >= node_pos.y) as usize,
                (pos.z >= node_pos.z) as usize,
            );
            let child_index = p.x * 4 + p.y * 2 + p.z;

            node_pos += (Vector3::new(p.x as f32, p.y as f32, p.z as f32) * 2.0
                - Vector3::new(1.0, 1.0, 1.0))
                / (1 << depth) as f32;

            if self.nodes[node_index + child_index] >= PALETTE_OFFSET {
                return (node_index + child_index, depth);
            }

            node_index = self.nodes[node_index + child_index] as usize;
        }
    }
}

// fn count_bits(mut n: u8) -> usize {
//     let mut count = 0;
//     while n != 0 {
//         n = n & (n - 1);
//         count += 1;
//     }
//     return count;
// }

impl std::fmt::Debug for CpuOctree {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "CpuOctree:\n")?;
        let mut c = 0;
        for value in &self.nodes {
            if *value >= PALETTE_OFFSET {
                let l = Leaf::unpack(*value);
                match l {
                    Some(l) => write!(f, "  Leaf: {}, {}, {}\n", l.r, l.g, l.b)?,
                    None => write!(f, "  Leaf: empty\n")?,
                }
            } else {
                write!(f, "  Node: {}\n", value)?;
            }

            c += 1;
            if c % 8 == 0 {
                write!(f, "\n")?;
            }
        }

        Ok(())
    }
}
// #endregion
