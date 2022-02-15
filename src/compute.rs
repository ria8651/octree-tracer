use super::*;

pub struct Compute {
    compute_pipeline: wgpu::ComputePipeline,
    bind_group: wgpu::BindGroup,
}

impl Compute {
    pub fn new(render: &Render) -> Self {
        let shader = render
            .device
            .create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(
                    (concat!(include_str!("common.wgsl"), include_str!("compute.wgsl"))).into(),
                ),
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

        let bind_group_layout = compute_pipeline.get_bind_group_layout(0);
        let bind_group = render.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: render.voxel_buffer.as_entire_binding(),
            }],
        });

        Self {
            compute_pipeline,
            bind_group,
        }
    }

    pub fn update(&self, render: &Render) {
        let mut encoder = render
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut cpass =
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });
            cpass.set_pipeline(&self.compute_pipeline);
            cpass.set_bind_group(0, &self.bind_group, &[]);
            cpass.insert_debug_marker("compute collatz iterations");
            cpass.dispatch(8, 1, 1); // Number of cells to run, the (x,y,z) size of item being processed
        }

        // Submits command encoder for processing
        render.queue.submit(Some(encoder.finish()));

        let buffer_slice = render.voxel_buffer.slice(..);
        let voxel_future = buffer_slice.map_async(wgpu::MapMode::Read);

        render.device.poll(wgpu::Maintain::Wait);

        if let Ok(()) = pollster::block_on(voxel_future) {
            {
                let data = buffer_slice.get_mapped_range();
                let result: Vec<u32> = bytemuck::cast_slice(&data).to_vec();

                for a in result {
                    println!("{}", a);
                }
            }

            render.voxel_buffer.unmap();
        } else {
            panic!("failed to run compute on gpu!")
        }
    }
}
