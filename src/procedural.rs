use super::*;

const WORK_GROUP_SIZE: u32 = 32;
const CHUNK_SIZE: usize = 100000000; // little less than the worst case for 2^8 octree 19173960
const ITERATIONS: u32 = 16777216; // (2^8)^3 16777216

pub struct GenSettings {
    pub seed: u32,
    pub scale: f32,
    pub height: f32,
}

impl Default for GenSettings {
    fn default() -> Self {
        GenSettings {
            seed: 0,
            scale: 0.2,
            height: 0.2,
        }
    }
}

pub struct Procedural {
    pipeline: wgpu::ComputePipeline,
    uniforms: Uniforms,
    uniform_buffer: wgpu::Buffer,
    cpu_octree: wgpu::Buffer,
    compute_bind_group: wgpu::BindGroup,
}

impl Procedural {
    pub fn new(gpu: &Gpu) -> Self {
        let shader = gpu
            .device
            .create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(
                    (concat!(include_str!("common.wgsl"), include_str!("procedual.wgsl"))).into(),
                ),
            });

        let dispatch_size = (ITERATIONS as f32 / WORK_GROUP_SIZE as f32).sqrt().ceil() as u32;
        let uniforms = Uniforms::new(dispatch_size, 8);
        let uniform_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Camera Buffer"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsages::UNIFORM
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
            });

        let pipeline = gpu
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: None,
                module: &shader,
                entry_point: "main",
            });

        let inital_octree = CpuOctree::new(255);
        let mut raw = inital_octree.raw();
        raw.insert(0, raw.len() as u64);
        raw.extend(std::iter::repeat(0).take(CHUNK_SIZE.checked_sub(raw.len()).unwrap()));

        let cpu_octree = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&raw),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::MAP_READ,
            });

        let compute_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: cpu_octree.as_entire_binding(),
                },
            ],
        });

        Self {
            pipeline,
            uniforms,
            uniform_buffer,
            cpu_octree,
            compute_bind_group,
        }
    }

    pub fn generate_chunk(&self, gpu: &Gpu, blocks: &Vec<CpuOctree>) -> CpuOctree {
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut compute_pass =
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

            let dispatch_size = (ITERATIONS as f32 / WORK_GROUP_SIZE as f32).sqrt().ceil() as u32;
            println!(
                "Dispatch size on x and y: {} (total threads: {})",
                dispatch_size,
                dispatch_size * dispatch_size * WORK_GROUP_SIZE
            );
            compute_pass.set_pipeline(&self.pipeline);
            compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);
            compute_pass.dispatch(dispatch_size, dispatch_size, 1);
        }

        gpu.queue.submit(Some(encoder.finish()));

        // Process output
        let mut cpu_octree = CpuOctree {
            nodes: Vec::new(),
            top_mip: Voxel::new(0, 0, 0),
        };

        let slice = self.cpu_octree.slice(..);
        let future = slice.map_async(wgpu::MapMode::Read);

        gpu.device.poll(wgpu::Maintain::Wait);

        if let Ok(()) = pollster::block_on(future) {
            let mut data = slice.get_mapped_range_mut();
            let result: &mut [u64] = unsafe { reinterpret::reinterpret_mut_slice(&mut data) };

            // Reset atomic counter
            let len = result[0] as usize;
            result[0] = 0;
            println!("Nodes recived from gpu: {}", len);

            for i in 1..=len {
                let pointer = (result[i] >> 32) as u32;
                if pointer == 0 {
                    cpu_octree
                        .nodes
                        .push(Node::new(BLOCK_OFFSET, Voxel::new(0, 0, 0)));
                } else {
                    let value = result[i] as u32;
                    cpu_octree
                        .nodes
                        .push(Node::new(pointer, Voxel::from_value(value)));
                }
            }

            drop(data);
            self.cpu_octree.unmap();
        } else {
            panic!("Failed to run get subdivision buffer!")
        }

        // println!("{:?}", cpu_octree);
        // panic!();

        cpu_octree.generate_mip_tree(Some(blocks));
        cpu_octree
    }
}

pub fn generate_world(
    gen_settings: &GenSettings,
    blocks: &Vec<CpuOctree>,
) -> Result<CpuOctree, String> {
    let mut octree = CpuOctree::new(0);

    let mut rng = RandomNumberGenerator::new();
    let mut terrain_noise = FastNoise::seeded(gen_settings.seed as u64);
    terrain_noise.set_noise_type(NoiseType::SimplexFractal);
    terrain_noise.set_fractal_type(FractalType::FBM);
    terrain_noise.set_fractal_octaves(5);
    terrain_noise.set_fractal_gain(0.6);
    terrain_noise.set_fractal_lacunarity(2.0);
    terrain_noise.set_frequency(2.0);

    let mut fracture_noise = FastNoise::seeded(gen_settings.seed as u64 + 5);
    fracture_noise.set_noise_type(NoiseType::Cellular);
    fracture_noise.set_cellular_distance_function(CellularDistanceFunction::Euclidean);
    fracture_noise.set_cellular_return_type(CellularReturnType::Distance2);
    fracture_noise.set_frequency(2.0);

    let tree_structure = CpuOctree::load_structure("structures/tree.vox".to_string());
    let crystal_structure = CpuOctree::load_structure("structures/crystal.vox".to_string());

    let world_depth = 8;
    let world_size = 1 << world_depth;
    let voxel_size = 2.0 / world_size as f32;
    for x in 0..world_size {
        for z in 0..world_size {
            let mut depth = 0;
            for y in (0..world_size).rev() {
                let mut pos = Vector3::new(x as f32, y as f32, z as f32);
                pos /= world_size as f32 / 2.0;
                pos -= Vector3::new(1.0, 1.0, 1.0);

                let mut v = terrain_noise.get_noise3d(
                    pos.x * gen_settings.scale,
                    pos.y * gen_settings.scale,
                    pos.z * gen_settings.scale,
                ) + 1.0;
                // let f = fracture_noise.get_noise3d(pos.x, pos.y, pos.z);

                // Height
                v *= gen_settings.height;

                // Edge of world
                let edge_distance = 0.5;
                let edge = (-pos.x.abs() + edge_distance)
                    .min(-pos.z.abs() + edge_distance)
                    .min(-pos.y.abs() + 0.0)
                    .min(0.0);
                v += edge;

                // Bottom of world
                let dist = (pos.x * pos.x + pos.z * pos.z).sqrt();
                {
                    let noise = terrain_noise.get_noise3d(pos.x * 0.3, pos.y * 0.1, pos.z * 0.3);
                    v += (-pos.y).clamp(0.0, 0.7) * (noise + (1.0 - 2.0 * dist));
                }

                if v > 0.0 {
                    if depth == 0 {
                        octree.put_in_block(pos, 3, world_depth, blocks);

                        if x == world_size / 2 && z == world_size / 2 {
                            for voxel in &crystal_structure {
                                let structure_pos = Vector3::new(
                                    voxel.0.x as f32,
                                    voxel.0.y as f32,
                                    voxel.0.z as f32,
                                ) * voxel_size;
                                octree.put_in_block(
                                    pos + structure_pos,
                                    voxel.1,
                                    world_depth,
                                    blocks,
                                );
                            }
                        } else if rng.range(0, 100) == 0 && dist > 0.2 {
                            for voxel in &tree_structure {
                                let structure_pos = Vector3::new(
                                    voxel.0.x as f32,
                                    voxel.0.y as f32,
                                    voxel.0.z as f32,
                                ) * voxel_size;
                                octree.put_in_block(
                                    pos + structure_pos,
                                    voxel.1,
                                    world_depth,
                                    blocks,
                                );
                            }
                        }
                    } else if depth < 5 {
                        octree.put_in_block(pos, 2, world_depth, blocks);
                    } else {
                        octree.put_in_block(pos, 1, world_depth, blocks);
                    }

                    depth += rng.range(1, 4);
                } else {
                    depth -= rng.range(1, 4);
                    depth = depth.max(0);
                }

                // if f > 0.1 {
                //     octree.put_in_block(pos, 4, world_depth, blocks);
                // }
            }
        }
    }

    println!("SVO size: {}", octree.nodes.len());

    octree.generate_mip_tree(Some(blocks));

    Ok(octree)
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Zeroable, bytemuck::Pod)]
struct Uniforms {
    dispatch_size: u32,
    depth: u32,
    misc1: f32,
    misc2: f32,
    misc3: f32,
}

impl Uniforms {
    fn new(dispatch_size: u32, depth: u32) -> Self {
        Self {
            dispatch_size,
            depth,
            misc1: 0.0,
            misc2: 0.0,
            misc3: 0.0,
        }
    }
}
