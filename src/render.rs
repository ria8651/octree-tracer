use super::*;

pub struct Render {
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub render_pipeline: wgpu::RenderPipeline,
    pub uniforms: Uniforms,
    pub uniform_buffer: wgpu::Buffer,
    pub node_buffer: wgpu::Buffer,
    pub main_bind_group: wgpu::BindGroup,
    pub previous_frame_time: Option<f64>,
    pub egui_platform: egui_winit_platform::Platform,
    pub egui_rpass: egui_wgpu_backend::RenderPass,
}

impl Render {
    // Creating some of the wgpu types requires async code
    pub async fn new(gpu: &Gpu, window: &Window, octree: &Octree) -> Self {
        window.set_cursor_grab(true).unwrap();
        window.set_cursor_visible(false);


        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: gpu.surface.get_preferred_format(&gpu.adapter).unwrap(),
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
        };
        gpu.surface.configure(&gpu.device, &config);

        let shader = gpu.device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(
                (concat!(include_str!("common.wgsl"), include_str!("shader.wgsl"))).into(),
            ),
        });

        // #region Buffers
        let uniforms = Uniforms::new();
        let uniform_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });

        let nodes = octree.expanded(10_000_000);

        let node_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&nodes),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let main_bind_group_layout =
            gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let main_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
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
            ],
            label: Some("uniform_bind_group"),
        });
        // #endregion

        let render_pipeline_layout =
            gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&main_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
        let egui_rpass = egui_wgpu_backend::RenderPass::new(&gpu.device, config.format, 1);

        let previous_frame_time = None;

        Self {
            config,
            size,
            render_pipeline,
            uniforms,
            uniform_buffer,
            node_buffer,
            main_bind_group,
            previous_frame_time,
            egui_platform,
            egui_rpass,
        }
    }

    pub fn resize(&mut self, gpu: &Gpu, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            gpu.surface.configure(&gpu.device, &self.config);
        }
    }

    pub fn update(&mut self, gpu: &Gpu, time: f64, settings: &mut Settings, character: &Character) {
        let dimensions = [self.size.width as f32, self.size.height as f32];

        let view = Matrix4::<f32>::look_at_rh(
            character.pos,
            character.pos + character.look,
            Vector3::unit_y(),
        );
        // let proj = perspective(Deg(settings.fov), dimensions[0] / dimensions[1], 0.00001, 0.0001);
        let proj = create_proj_matrix(settings.fov, dimensions[1] / dimensions[0]);
        let camera = proj * view;
        let camera_inverse = camera.invert().unwrap();

        self.uniforms.dimensions = [dimensions[0], dimensions[1], 0.0, 0.0];
        self.uniforms.camera = camera.into();
        self.uniforms.camera_inverse = camera_inverse.into();

        gpu.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.uniforms]),
        );

        self.egui_platform.update_time(time);
    }

    pub fn render(&mut self, gpu: &Gpu, window: &Window) -> Result<(), wgpu::SurfaceError> {
        let output = gpu.surface.get_current_texture()?;
        let size = window.inner_size();

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = gpu
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
            &gpu.device,
            &gpu.queue,
            &self.egui_platform.context().font_image(),
        );
        self.egui_rpass
            .update_user_textures(&gpu.device, &gpu.queue);
        self.egui_rpass
            .update_buffers(&gpu.device, &gpu.queue, &paint_jobs, &screen_descriptor);

        // Record all render passes.
        self.egui_rpass
            .execute(&mut encoder, &view, &paint_jobs, &screen_descriptor, None)
            .unwrap();

        // Submit the command buffer.
        gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Zeroable)]
pub struct Uniforms {
    pub camera: [[f32; 4]; 4],
    pub camera_inverse: [[f32; 4]; 4],
    pub dimensions: [f32; 4],
    pub sun_dir: [f32; 4],
    pub pause_adaptive: bool,
    pub show_steps: bool,
    pub show_hits: bool,
    pub shadows: bool,
    pub misc_value: f32,
    pub misc_bool: bool,
    pub junk: [u32; 8],
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