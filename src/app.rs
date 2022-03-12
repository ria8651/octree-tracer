use super::*;
use winit::window::Window;

pub struct App {
    pub octree: Octree,
    pub world: World,
    pub gen_settings: GenSettings,
    pub gpu: Gpu,
    pub render: Render,
    pub compute: Compute,
    pub procedural: Procedural,
    pub input: Input,
    pub character: Character,
    pub settings: Settings,
    ui: Ui,
}

impl App {
    pub async fn new(window: &Window) -> Self {
        let input = Input::new();
        let character = Character::new();

        let settings = Settings {
            octree_depth: 12,
            fov: 90.0,
            sensitivity: 0.00005,
        };

        let gpu = Gpu::new(window).await;
        let procedural = Procedural::new(&gpu);

        let world = World::load_world("worlds/defualt").unwrap();
        for (index, chunk) in &world.chunks {
            println!("Chunk {} (Nodes: {})", index, chunk.nodes.len());
        }

        let gen_settings = GenSettings::default();
        // let cpu_octree = generate_world(&gen_settings, &blocks).unwrap();
        // let defualt_octree = CpuOctree::new(0b01011011);
        // let cpu_octree = match CpuOctree::load_file(octree_path, octree_depth, Some(&blocks)) {
        //     Ok(cpu_octree) => cpu_octree,
        //     Err(_) => defualt_octree,
        // };

        let mask = world.chunks[&0].get_node_mask(0);
        let octree = Octree::new(mask);
        // let octree = world.to_octree();

        let render = Render::new(&gpu, window, &octree).await;
        let compute = Compute::new(&gpu, &render);

        let app = Self {
            octree,
            world,
            gen_settings,
            gpu,
            render,
            compute,
            procedural,
            input,
            character,
            settings,
            ui: Default::default(),
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
                &mut self.octree,
                &mut self.world,
            );
            process_unsubdivision(
                &mut self.compute,
                &self.gpu,
                &mut self.octree,
                &mut self.world,
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
                    ui.horizontal(|ui| {
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
                                ) {
                                    Ok(chunk) => {
                                        self.world.chunks.remove(&0);
                                        self.world.chunks.insert(0, chunk);
                                        self.world.generate_mip_tree(0);

                                        // Reset octree
                                        let mask = self.world.chunks[&0].get_node_mask(0);
                                        self.octree = Octree::new(mask);

                                        let nodes = self.octree.raw_data();
                                        self.gpu.queue.write_buffer(
                                            &self.render.node_buffer,
                                            0,
                                            bytemuck::cast_slice(&nodes),
                                        );

                                        self.ui.error_string = "".to_string();
                                    }
                                    Err(e) => {
                                        self.ui.error_string = e;
                                    }
                                },
                                None => self.ui.error_string = "No file selected".to_string(),
                            }
                        }

                        if ui.button("Open World").clicked() {
                            let path = native_dialog::FileDialog::new()
                                .add_filter("Bin in world folder", &["bin"])
                                .show_open_single_file()
                                .unwrap();

                            match path {
                                Some(path) => {
                                    self.world = World::load_world(path.parent().unwrap()).unwrap();

                                    // Reset octree
                                    let mask = self.world.chunks[&0].get_node_mask(0);
                                    self.octree = Octree::new(mask);

                                    let nodes = self.octree.raw_data();
                                    self.gpu.queue.write_buffer(
                                        &self.render.node_buffer,
                                        0,
                                        bytemuck::cast_slice(&nodes),
                                    );

                                    self.ui.error_string = "".to_string();
                                }
                                None => self.ui.error_string = "No file selected".to_string(),
                            }
                        }

                        if ui.button("Save File").clicked() {
                            let path = native_dialog::FileDialog::new()
                                .show_save_single_file()
                                .unwrap();

                            match path {
                                Some(path) => match self.world.save_world(path) {
                                    Ok(_) => self.ui.error_string = "".to_string(),
                                    Err(e) => self.ui.error_string = e,
                                },
                                None => self.ui.error_string = "No file selected".to_string(),
                            }
                        }

                        if ui.button("Regenerate").clicked() {
                            let path = native_dialog::FileDialog::new()
                                .show_save_single_file()
                                .unwrap();

                            match path {
                                Some(path) => {
                                    World::generate_world(&path, &mut self.procedural, &self.gpu)
                                        .unwrap();

                                    self.world = World::load_world(path).unwrap();

                                    // Reset octree
                                    let mask = self.world.chunks[&0].get_node_mask(0);
                                    self.octree = Octree::new(mask);

                                    let nodes = self.octree.raw_data();
                                    self.gpu.queue.write_buffer(
                                        &self.render.node_buffer,
                                        0,
                                        bytemuck::cast_slice(&nodes),
                                    );

                                    self.ui.error_string = "".to_string();
                                }
                                None => self.ui.error_string = "No file selected".to_string(),
                            }
                        }
                    });

                    if self.ui.error_string != "" {
                        ui.colored_label(egui::Color32::RED, &self.ui.error_string);
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

            // fn update_world_gen(app: &mut App) {
            //     app.world = World::generate_world(&app.procedural, &app.gpu, &app.blocks);;\
            //     app.octree = Octree::new(app.world.get_node_mask(0));
            // }

            // egui::CollapsingHeader::new("World gen")
            //     .default_open(false)
            //     .show(ui, |ui| {
            //         if ui
            //             .add(
            //                 egui::Slider::new(&mut self.procedural.uniforms.misc1, 0.0..=1.0)
            //                     .prefix("Misc1: "),
            //             )
            //             .changed()
            //         {
            //             update_world_gen(self)
            //         }
            //         if ui
            //             .add(
            //                 egui::Slider::new(&mut self.procedural.uniforms.misc2, 0.0..=10.0)
            //                     .prefix("Misc2: "),
            //             )
            //             .changed()
            //         {
            //             update_world_gen(self)
            //         }
            //         if ui
            //             .add(
            //                 egui::Slider::new(&mut self.procedural.uniforms.misc3, 0.0..=16777216.0)
            //                     .prefix("Misc3: "),
            //             )
            //             .changed()
            //         {
            //             update_world_gen(self)
            //         }
            //     });
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

#[derive(Default)]
struct Ui {
    error_string: String,
    save_file_name: String,
}
