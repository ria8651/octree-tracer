use super::*;
use winit::window::Window;

pub struct App {
    pub octree: Octree,
    pub cpu_octree: CpuOctree,
    pub gen_settings: GenSettings,
    pub blocks: Vec<CpuOctree>,
    pub gpu: Gpu,
    pub render: Render,
    pub compute: Compute,
    pub procedural: Procedural,
    pub input: Input,
    pub character: Character,
    pub settings: Settings,
}

impl App {
    pub async fn new(window: &Window, octree_path: String, octree_depth: u32) -> Self {
        let input = Input::new();
        let character = Character::new();
        let error_string = "".to_string();

        let settings = Settings {
            octree_depth,
            fov: 90.0,
            sensitivity: 0.00005,
            error_string,
        };

        let blocks = vec![
            // The empty block
            CpuOctree::new(0),
            CpuOctree::load_file("blocks/stone.vox".to_string(), 0, None).unwrap(),
            CpuOctree::load_file("blocks/dirt.vox".to_string(), 0, None).unwrap(),
            CpuOctree::load_file("blocks/grass.vox".to_string(), 0, None).unwrap(),
            CpuOctree::load_file("blocks/wood.vox".to_string(), 0, None).unwrap(),
            CpuOctree::load_file("blocks/leaf.vox".to_string(), 0, None).unwrap(),
            CpuOctree::load_file("blocks/slate.vox".to_string(), 0, None).unwrap(),
            CpuOctree::load_file("blocks/crystal.vox".to_string(), 0, None).unwrap(),
            CpuOctree::load_file("blocks/glass.vox".to_string(), 0, None).unwrap(),
        ];

        let gpu = Gpu::new(window).await;
        let procedural = Procedural::new(&gpu);

        let cpu_octree = procedural.generate_chunk(&gpu, &blocks);

        let gen_settings = GenSettings::default();
        // let cpu_octree = generate_world(&gen_settings, &blocks).unwrap();
        // let defualt_octree = CpuOctree::new(0b01011011);
        // let cpu_octree = match CpuOctree::load_file(octree_path, octree_depth, Some(&blocks)) {
        //     Ok(cpu_octree) => cpu_octree,
        //     Err(_) => defualt_octree,
        // };

        let mask = cpu_octree.get_node_mask(0);
        let octree = Octree::new(mask);
        // let octree = cpu_octree.to_octree();

        let render = Render::new(&gpu, window, &octree, octree_depth).await;
        let compute = Compute::new(&gpu, &render, octree_depth);

        let app = Self {
            octree,
            cpu_octree,
            gen_settings,
            blocks,
            gpu,
            render,
            compute,
            procedural,
            input,
            character,
            settings,
        };

        app
    }

    pub fn update(&mut self, time: f64) {
        self.gui(time);

        let input = Vector3::new(
            self.input.right as u32 as f32 - self.input.left as u32 as f32,
            self.input.up as u32 as f32 - self.input.down as u32 as f32,
            self.input.forward as u32 as f32 - self.input.backward as u32 as f32,
        ) * std::f32::consts::E.powf(self.character.speed);

        let forward: Vector3<f32> = self.character.look.normalize();
        let right = forward.cross(Vector3::unit_y()).normalize();
        let up = right.cross(forward);

        self.character.pos += forward * input.z + right * input.x + up * input.y;

        if self.character.cursour_grabbed {
            let delta = self.settings.sensitivity * self.input.mouse_delta * self.settings.fov;
            let rotation = Quaternion::from_axis_angle(right, Rad(-delta.y))
                * Quaternion::from_axis_angle(Vector3::unit_y(), Rad(-delta.x));

            self.input.mouse_delta = Vector2::zero();
            self.character.look = (rotation * self.character.look).normalize();
        }

        self.render
            .update(&self.gpu, time, &mut self.settings, &self.character);

        if !self.render.uniforms.pause_adaptive {
            self.compute.update(&self.gpu, &self.octree);

            process_subdivision(
                &mut self.compute,
                &self.gpu,
                &mut self.render,
                &mut self.octree,
                &self.cpu_octree,
                &self.blocks,
            );
            process_unsubdivision(
                &mut self.compute,
                &self.gpu,
                &mut self.render,
                &mut self.octree,
                &self.cpu_octree,
                &self.blocks,
            );

            // Write octree to gpu
            let nodes = self.octree.raw_data();

            self.gpu
                .queue
                .write_buffer(&self.render.node_buffer, 0, bytemuck::cast_slice(&nodes));
        }
    }

    pub fn gui(&mut self, time: f64) {
        let fps = if let Some(previous_frame_time) = self.render.previous_frame_time {
            let fps = 1.0 / (time - previous_frame_time);
            self.render.previous_frame_time = Some(time);
            fps
        } else {
            self.render.previous_frame_time = Some(time);
            0.0
        };

        let hole_percentage =
            100.0 * (8.0 * self.octree.hole_stack.len() as f32) / self.octree.nodes.len() as f32;

        egui::Window::new("Info").show(&self.render.egui_platform.context(), |ui| {
            ui.label(format!("FPS: {:.0}", fps));
            egui::CollapsingHeader::new("Render")
                .default_open(true)
                .show(ui, |ui| {
                    if ui.button("Open File").clicked() {
                        let path = native_dialog::FileDialog::new()
                            .add_filter("Magica Voxel RSVO File", &["rsvo"])
                            .add_filter("Magica Voxel Vox File", &["vox"])
                            .show_open_single_file()
                            .unwrap();

                        match path {
                            Some(path) => match CpuOctree::load_file(
                                path.into_os_string().into_string().unwrap(),
                                self.settings.octree_depth,
                                Some(&self.blocks),
                            ) {
                                Ok(cpu_octree) => {
                                    self.cpu_octree = cpu_octree;

                                    // Reset octree
                                    let mask = self.cpu_octree.get_node_mask(0);
                                    self.octree = Octree::new(mask);

                                    let nodes = self.octree.raw_data();
                                    self.gpu.queue.write_buffer(
                                        &self.render.node_buffer,
                                        0,
                                        bytemuck::cast_slice(&nodes),
                                    );

                                    self.render.uniforms.max_depth = self.settings.octree_depth;
                                    self.settings.error_string = "".to_string();
                                }
                                Err(e) => {
                                    self.settings.error_string = e;
                                }
                            },
                            None => self.settings.error_string = "No file selected".to_string(),
                        }
                    }
                    if self.settings.error_string != "" {
                        ui.colored_label(egui::Color32::RED, &self.settings.error_string);
                    }

                    ui.add(
                        egui::Slider::new(&mut self.settings.octree_depth, 0..=20)
                            .text("Octree depth"),
                    );

                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut self.render.uniforms.sun_dir[0])
                                .speed(0.05)
                                .prefix("x: "),
                        );
                        ui.add(
                            egui::DragValue::new(&mut self.render.uniforms.sun_dir[1])
                                .speed(0.05)
                                .prefix("y: "),
                        );
                        ui.add(
                            egui::DragValue::new(&mut self.render.uniforms.sun_dir[2])
                                .speed(0.05)
                                .prefix("z: "),
                        );
                    });

                    ui.checkbox(&mut self.render.uniforms.show_steps, "Show ray steps");
                    ui.checkbox(&mut self.render.uniforms.show_hits, "Show ray hits");
                    ui.checkbox(&mut self.render.uniforms.shadows, "Shadows");
                    ui.checkbox(&mut self.render.uniforms.pause_adaptive, "Pause adaptive");
                    ui.add(
                        egui::Slider::new(&mut self.render.uniforms.misc_value, 0.0000001..=0.0001)
                            .text("Misc")
                            .logarithmic(true),
                    );
                    ui.checkbox(&mut self.render.uniforms.misc_bool, "Misc");

                    ui.label(format!(
                        "Nodes: {:.2} million ({:.0}% holes)",
                        self.octree.nodes.len() as f32 / 1000000.0,
                        hole_percentage,
                    ));
                });

            egui::CollapsingHeader::new("Character")
                .default_open(false)
                .show(ui, |ui| {
                    ui.add(
                        egui::Slider::new(&mut self.settings.fov, 0.01..=100.0)
                            .prefix("FOV: ")
                            .logarithmic(true),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.settings.sensitivity, 0.00001..=0.0001)
                            .prefix("Sensitivity")
                            .logarithmic(true),
                    );
                });

            fn update_world_gen(app: &mut App) {
                app.cpu_octree = generate_world(&app.gen_settings, &app.blocks).unwrap();
                app.octree = Octree::new(app.cpu_octree.get_node_mask(0));
            }

            egui::CollapsingHeader::new("World gen")
                .default_open(false)
                .show(ui, |ui| {
                    if ui
                        .add(
                            egui::Slider::new(&mut self.gen_settings.seed, 0..=u32::MAX)
                                .prefix("Seed: "),
                        )
                        .changed()
                    {
                        update_world_gen(self)
                    }
                    if ui
                        .add(
                            egui::Slider::new(&mut self.gen_settings.scale, 0.01..=1.0)
                                .prefix("Scale: "),
                        )
                        .changed()
                    {
                        update_world_gen(self)
                    }
                    if ui
                        .add(
                            egui::Slider::new(&mut self.gen_settings.height, 0.01..=1.0)
                                .prefix("Height: "),
                        )
                        .changed()
                    {
                        update_world_gen(self)
                    }
                });
        });
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
                DeviceEvent::MouseWheel {
                    delta:
                        winit::event::MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition {
                            y,
                            ..
                        }),
                    ..
                } => {
                    self.character.speed += *y as f32 / 200.0;
                }
                _ => {}
            },
            _ => {}
        }
    }
}
