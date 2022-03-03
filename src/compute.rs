use super::*;

const WORK_GROUP_SIZE: u32 = 16;
const DISPATCH_SIZE_Y: u32 = 256;

pub struct Compute {
    pipeline: wgpu::ComputePipeline,
    uniforms: Uniforms,
    uniform_buffer: wgpu::Buffer,
    pub subdivision_buffer: wgpu::Buffer,
    pub unsubdivision_buffer: wgpu::Buffer,
    compute_bind_group: wgpu::BindGroup,
}

impl Compute {
    pub fn new(gpu: &Gpu, render: &Render) -> Self {
        let shader = gpu
            .device
            .create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(
                    (concat!(include_str!("common.wgsl"), include_str!("compute.wgsl"))).into(),
                ),
            });

        let uniforms = Uniforms::new(0);
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

        let subdivision_buffer =
            gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&[0u32; MAX_SUBDIVISIONS_PER_FRAME]),
                    usage: wgpu::BufferUsages::STORAGE
                        | wgpu::BufferUsages::COPY_DST
                        | wgpu::BufferUsages::MAP_READ,
                });

        let unsubdivision_buffer =
            gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&[0u32; MAX_UNSUBDIVISIONS_PER_FRAME]),
                    usage: wgpu::BufferUsages::STORAGE
                        | wgpu::BufferUsages::COPY_DST
                        | wgpu::BufferUsages::MAP_READ,
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
                    resource: render.node_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: subdivision_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: unsubdivision_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            pipeline,
            uniforms,
            uniform_buffer,
            subdivision_buffer,
            unsubdivision_buffer,
            compute_bind_group,
        }
    }

    pub fn update(&mut self, gpu: &Gpu, octree: &Octree) {
        let iterations = octree.nodes.len();
        let dispatch_size_x =
            (iterations as f32 / WORK_GROUP_SIZE as f32 / DISPATCH_SIZE_Y as f32).ceil() as u32;

        // println!("Iteration: {} / Dispatch size: {}", iterations, dispatch_size_x);

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut compute_pass =
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

            compute_pass.set_pipeline(&self.pipeline);
            compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);
            compute_pass.dispatch(dispatch_size_x, DISPATCH_SIZE_Y, 1);
        }

        self.uniforms.node_length = octree.nodes.len() as u32;
        gpu.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.uniforms]),
        );

        gpu.queue.submit(Some(encoder.finish()));
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Zeroable, bytemuck::Pod)]
struct Uniforms {
    node_length: u32,
    max_depth: u32,
}

impl Uniforms {
    fn new(max_depth: u32) -> Self {
        Self {
            node_length: 0,
            max_depth,
        }
    }
}
