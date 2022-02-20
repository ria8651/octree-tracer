use super::*;

const WORK_GROUP_SIZE: u32 = 16;
const DISPATCH_SIZE_Y: u32 = 256;

pub struct Compute {
    compute_pipeline: wgpu::ComputePipeline,
    c_uniforms: CUniforms,
    c_uniform_buffer: wgpu::Buffer,
    pub subdivision_buffer: wgpu::Buffer,
    pub unsubdivision_buffer: wgpu::Buffer,
    compute_bind_group: wgpu::BindGroup,
}

impl Compute {
    pub fn new(render: &Render, max_depth: u32) -> Self {
        let shader = render
            .device
            .create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(
                    (concat!(include_str!("common.wgsl"), include_str!("compute.wgsl"))).into(),
                ),
            });

        let c_uniforms = CUniforms::new(max_depth);
        let c_uniform_buffer =
            render
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Camera Buffer"),
                    contents: bytemuck::cast_slice(&[c_uniforms]),
                    usage: wgpu::BufferUsages::UNIFORM
                        | wgpu::BufferUsages::COPY_DST
                        | wgpu::BufferUsages::COPY_SRC,
                });

        let compute_pipeline =
            render
                .device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: None,
                    layout: None,
                    module: &shader,
                    entry_point: "main",
                });

        let subdivision_buffer =
            render
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&[0u32; MAX_SUBDIVISIONS_PER_FRAME]),
                    usage: wgpu::BufferUsages::STORAGE
                        | wgpu::BufferUsages::COPY_DST
                        | wgpu::BufferUsages::MAP_READ,
                });

        let unsubdivision_buffer =
            render
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&[0u32; MAX_UNSUBDIVISIONS_PER_FRAME]),
                    usage: wgpu::BufferUsages::STORAGE
                        | wgpu::BufferUsages::COPY_DST
                        | wgpu::BufferUsages::MAP_READ,
                });

        let compute_bind_group = render.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &compute_pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: c_uniform_buffer.as_entire_binding(),
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
            compute_pipeline,
            c_uniforms,
            c_uniform_buffer,
            subdivision_buffer,
            unsubdivision_buffer,
            compute_bind_group,
        }
    }

    pub fn update(&mut self, render: &Render, octree: &Octree) {
        let iterations = octree.nodes.len();
        let dispatch_size_x =
            (iterations as f32 / WORK_GROUP_SIZE as f32 / DISPATCH_SIZE_Y as f32).ceil() as u32;

        // println!("Iteration: {} / Dispatch size: {}", iterations, dispatch_size_x);

        let mut encoder = render
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut compute_pass =
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &self.compute_bind_group, &[]);
            compute_pass.dispatch(dispatch_size_x, DISPATCH_SIZE_Y, 1);
        }

        self.c_uniforms.node_length = octree.nodes.len() as u32;
        render.queue.write_buffer(
            &self.c_uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.c_uniforms]),
        );

        render.queue.submit(Some(encoder.finish()));
    }
}
