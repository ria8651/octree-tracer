use super::*;
use winit::window::Window;

pub struct App {
    pub octree: Octree,
    pub cpu_octree: Octree,
    pub render: Render,
    pub compute: Compute,
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

        let octree = Octree::new(0b11000101);

        let mut defualt_octree = Octree::new(0);
        defualt_octree.put_in_voxel(Vector3::new(1.0, 1.0, 1.0), 1, 3);
        defualt_octree.put_in_voxel(Vector3::new(0.0, 0.0, 0.0), 1, 3);
        defualt_octree.put_in_voxel(Vector3::new(-1.0, -1.0, -1.0), 1, 3);

        let cpu_octree = match load_file(octree_path, octree_depth) {
            Ok(cpu_octree) => cpu_octree,
            Err(_) => defualt_octree,
        };

        // So we can load a bigger octree later
        // octree.expand(256000000);

        let render = Render::new(window, &octree).await;
        let compute = Compute::new(&render);

        let app = Self {
            octree,
            cpu_octree,
            render,
            compute,
            input,
            character,
            settings,
        };

        // octree.subdivide(0, 0b0100101, true, 2);

        // app.render.update(0.0, &mut app.settings, &app.character);
        // app.render.render(&window).unwrap();

        // app.compute
        //     .update(&app.render, &mut app.octree, &mut app.cpu_octree);
        // app.update(0.0);
        // println!("{:?}", app.octree);
        // panic!();

        app
    }

    pub fn update(&mut self, time: f64) {
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

        let fps = if let Some(previous_frame_time) = self.render.previous_frame_time {
            let fps = 1.0 / (time - previous_frame_time);
            self.render.previous_frame_time = Some(time);
            fps
        } else {
            self.render.previous_frame_time = Some(time);
            0.0
        };

        egui::Window::new("Info").show(&self.render.egui_platform.context(), |ui| {
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
                        self.settings.octree_depth,
                    ) {
                        Ok(octree) => {
                            self.octree = octree;

                            let (nodes, voxels) = self.octree.raw_data();

                            self.render.queue.write_buffer(
                                &self.render.node_buffer,
                                0,
                                bytemuck::cast_slice(&nodes),
                            );
                            self.render.queue.write_buffer(
                                &self.render.voxel_buffer,
                                0,
                                bytemuck::cast_slice(&voxels),
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

            ui.add(egui::Slider::new(&mut self.settings.octree_depth, 0..=20).text("Octree depth"));
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
            ui.checkbox(&mut self.render.uniforms.shadows, "Shadows");
            ui.add(egui::Slider::new(&mut self.render.uniforms.misc_value, 0.0..=1.0).text("Misc"));
            ui.checkbox(&mut self.render.uniforms.misc_bool, "Misc");
        });

        self.render
            .update(time, &mut self.settings, &self.character);
        self.compute
            .update(&self.render, &mut self.octree, &mut self.cpu_octree);

        // Write octree to gpu
        let (nodes, voxels) = self.octree.raw_data();

        self.render
            .queue
            .write_buffer(&self.render.node_buffer, 0, bytemuck::cast_slice(&nodes));
        self.render
            .queue
            .write_buffer(&self.render.voxel_buffer, 0, bytemuck::cast_slice(&voxels));
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
                    delta: winit::event::MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition { y, ..}),
                    ..
                } => {
                    println!("{:?}", *y);
                    self.character.speed += *y as f32 / 200.0;
                }
                _ => {}
            },
            _ => {}
        }
    }
}
