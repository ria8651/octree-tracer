use super::*;

const MAX_SIBDIVISIONS: usize = 128000;
const WORK_GROUP_SIZE: u32 = 128;

pub struct Compute {
    compute_pipeline: wgpu::ComputePipeline,
    voxel_bind_group: wgpu::BindGroup,
    feedback_buffer: wgpu::Buffer,
    feedback_bind_group: wgpu::BindGroup,
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

        let feedback_buffer = render
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&[0; MAX_SIBDIVISIONS]),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::MAP_READ,
            });

        let voxel_bind_group = render.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &compute_pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: render.voxel_buffer.as_entire_binding(),
            }],
        });

        let feedback_bind_group = render.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &compute_pipeline.get_bind_group_layout(1),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: feedback_buffer.as_entire_binding(),
            }],
        });

        Self {
            compute_pipeline,
            voxel_bind_group,
            feedback_buffer,
            feedback_bind_group,
        }
    }

    pub fn update(&self, render: &Render, octree: &mut Octree, cpu_octree: &mut Octree) {
        let iterations = octree.voxel_len() as u32;
        let work_group_invocations = (iterations as f32 / WORK_GROUP_SIZE as f32).ceil() as u32;

        let mut encoder = render
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut compute_pass =
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None });

            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &self.voxel_bind_group, &[]);
            compute_pass.set_bind_group(1, &self.feedback_bind_group, &[]);
            compute_pass.dispatch(work_group_invocations, 1, 1);
        }

        render.queue.submit(Some(encoder.finish()));

        let feedback_slice = self.feedback_buffer.slice(..);
        let feedback_future = feedback_slice.map_async(wgpu::MapMode::Read);

        render.device.poll(wgpu::Maintain::Wait);

        if let Ok(()) = pollster::block_on(feedback_future) {
            {
                let mut data = feedback_slice.get_mapped_range_mut();
                let result: &mut [u32] = unsafe { reinterpret::reinterpret_mut_slice(&mut data) };

                let len = result[0] as usize;
                result[0] = 0;
                for i in 1..=len {
                    println!("subdivide: {:?}", result[i]);
                    let pos = octree.voxel_positions[result[i] as usize];
                    Compute::subdivide_octree(pos, octree, cpu_octree);
                    result[i] = 0;
                }
            }

            self.feedback_buffer.unmap();
        } else {
            panic!("failed to run compute on gpu!")
        }
    }

    pub fn subdivide_octree(pos: Vector3<f32>, octree: &mut Octree, cpu_octree: &mut Octree) {
        if pos == Vector3::zero() {
            panic!("Tried to subdivide deleted node!");
        }
        // println!("subdivide: {:?}", pos);

        let (voxel_index, voxel_depth, voxel_pos) = octree.get_node(pos);
        if voxel_depth < 20 {
            octree.subdivide(voxel_index, 0b10001001, true, voxel_depth + 1);
        }
        // let (cpu_octree_node, cpu_octree_depth, cpu_octree_pos) = app.cpu_octree.get_node(pos);
    }
}
