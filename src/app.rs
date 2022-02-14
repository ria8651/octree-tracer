use super::*;
use winit::{
    event::*,
    window::Window,
};

pub struct App {
    pub surface: wgpu::Surface,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub render_pipeline: wgpu::RenderPipeline,
    pub uniforms: Uniforms,
    pub uniform_buffer: wgpu::Buffer,
    pub node_buffer: wgpu::Buffer,
    pub voxel_buffer: wgpu::Buffer,
    // pub main_bind_group_layout: wgpu::BindGroupLayout,
    pub main_bind_group: wgpu::BindGroup,
    pub input: Input,
    pub character: Character,
    pub previous_frame_time: Option<f64>,
    pub egui_platform: egui_winit_platform::Platform,
    pub egui_rpass: egui_wgpu_backend::RenderPass,
    pub settings: Settings,
}

impl App {
    // Creating some of the wgpu types requires async code
    pub async fn new(window: &Window, svo_path: String, svo_depth: u32) -> Self {
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
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        });

        let mut defualt_octree = CpuOctree::new(0);
        defualt_octree.put_in_voxel(Vector3::new(1.0, 1.0, 1.0), Leaf::new(1), 3);
        defualt_octree.put_in_voxel(Vector3::new(0.0, 0.0, 0.0), Leaf::new(1), 3);
        defualt_octree.put_in_voxel(Vector3::new(-1.0, -1.0, -1.0), Leaf::new(1), 3);

        let mut svo = match load_file(svo_path, svo_depth) {
            Ok(svo) => svo,
            Err(_) => defualt_octree,
        };

        let svo = svo.raw_data();

        // So we can load a bigger file later
        svo.extend(std::iter::repeat(0).take(256000000 - svo.len()));

        let node_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("DF Buffer"),
            contents: bytemuck::cast_slice(&svo),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });
        let voxel_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("DF Buffer"),
            contents: bytemuck::cast_slice(&svo),
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
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
                    resource: node_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: voxel_buffer.as_entire_binding(),
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
            node_buffer,
            voxel_buffer,
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

    pub fn input(&mut self, window: &Window, event: &Event<()>) {
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
                        if *state == ElementState::Pressed {
                            window.set_cursor_visible(self.character.cursour_grabbed);
                            self.character.cursour_grabbed = !self.character.cursour_grabbed;
                            window
                                .set_cursor_grab(self.character.cursour_grabbed)
                                .unwrap();
                        }
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

    pub fn update(&mut self, window: &Window, time: f64) {
        let input = Vector3::new(
            self.input.right as u32 as f32 - self.input.left as u32 as f32,
            self.input.up as u32 as f32 - self.input.down as u32 as f32,
            self.input.forward as u32 as f32 - self.input.backward as u32 as f32,
        ) * 0.01;

        let forward: Vector3<f32> = self.character.look.normalize();
        let right = forward.cross(Vector3::unit_y()).normalize();
        let up = right.cross(forward);

        self.character.pos += forward * input.z + right * input.x + up * input.y;

        if self.character.cursour_grabbed {
            let delta = self.settings.sensitivity * self.input.mouse_delta;
            let rotation = Quaternion::from_axis_angle(right, Rad(-delta.y))
                * Quaternion::from_axis_angle(Vector3::unit_y(), Rad(-delta.x));
            self.input.mouse_delta = Vector2::zero();
            self.character.look = (rotation * self.character.look).normalize();
        }

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
                        Ok(mut svo) => {
                            self.queue.write_buffer(
                                &self.node_buffer,
                                0,
                                bytemuck::cast_slice(&svo.raw_data()),
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
            ui.add(
                egui::Slider::new(&mut self.settings.fov, 0.01..=100.0)
                    .prefix("FOV: ")
                    .logarithmic(true),
            );
            ui.add(
                egui::Slider::new(&mut self.settings.sensitivity, 0.00001..=0.01).prefix("Sensitivity").logarithmic(true),
            );

            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut self.uniforms.sun_dir[0])
                        .speed(0.05)
                        .prefix("x: "),
                );
                ui.add(
                    egui::DragValue::new(&mut self.uniforms.sun_dir[1])
                        .speed(0.05)
                        .prefix("y: "),
                );
                ui.add(
                    egui::DragValue::new(&mut self.uniforms.sun_dir[2])
                        .speed(0.05)
                        .prefix("z: "),
                );
            });

            ui.checkbox(&mut self.uniforms.show_steps, "Show ray steps");
            ui.checkbox(&mut self.uniforms.shadows, "Shadows");
            ui.add(egui::Slider::new(&mut self.uniforms.misc_value, 0.0..=0.01).text("Misc"));
            ui.checkbox(&mut self.uniforms.misc_bool, "Misc");
        });

        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.uniforms]),
        );

        self.egui_platform.update_time(time);
    }

    pub fn render(&mut self, window: &Window) -> Result<(), wgpu::SurfaceError> {
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