use super::*;

// TODO: There isn't 256 million pixels, why do lower values crash?
pub const MAX_SUBDIVISIONS_PER_FRAME: usize = 1024000;
pub const MAX_UNSUBDIVISIONS_PER_FRAME: usize = 1024000;

pub fn process_subdivision(
    compute: &mut Compute,
    render: &mut Render,
    octree: &mut Octree,
    cpu_octree: &CpuOctree,
    blocks: &Vec<CpuOctree>,
) {
    let slice = compute.subdivision_buffer.slice(..);
    let future = slice.map_async(wgpu::MapMode::Read);

    render.device.poll(wgpu::Maintain::Wait);

    if let Ok(()) = pollster::block_on(future) {
        let mut data = slice.get_mapped_range_mut();
        let result: &mut [u32] = unsafe { reinterpret::reinterpret_mut_slice(&mut data) };

        // Reset atomic counter
        let len = (result[0] as usize).min(MAX_SUBDIVISIONS_PER_FRAME - 1);
        result[0] = 0;

        // if len > 0 {
        //     println!("Processing {} subdivisions", len);
        // }
        for i in 1..=len {
            let node_index = result[i] as usize;

            if octree.get_node(node_index) < VOXEL_OFFSET {
                println!("Doubleup!");
                continue;
            }

            let pos = octree.positions[node_index];
            let (_, voxel_depth, _) = octree.find_voxel(pos, None);
            let (cpu_index, cpu_depth, cpu_pos) =
                cpu_octree.find_voxel(pos, Some(voxel_depth), None);

            let tnipt = cpu_octree.nodes[cpu_index];
            if tnipt.pointer < BLOCK_OFFSET {
                let mask = cpu_octree.get_node_mask(tnipt.pointer as usize);
                octree.subdivide(node_index, mask, voxel_depth + 1);
            } else if tnipt.pointer > BLOCK_OFFSET {
                let block_index = tnipt.pointer as usize - BLOCK_OFFSET as usize;
                let block = &blocks[block_index];

                if voxel_depth == cpu_depth {
                    let mask = block.get_node_mask(0);
                    octree.subdivide(node_index, mask, voxel_depth + 1);
                } else {
                    let voxel_size = 2.0 / (1 << (cpu_depth + 1)) as f32;
                    let block_pos = (pos - cpu_pos) / voxel_size;

                    let (block_index, _, _) =
                        block.find_voxel(block_pos, Some(voxel_depth - cpu_depth), None);

                    let tnipt = block.nodes[block_index];
                    if tnipt.pointer < BLOCK_OFFSET {
                        let mask = block.get_node_mask(tnipt.pointer as usize);
                        octree.subdivide(node_index, mask, voxel_depth + 1);
                    }
                }
            }

            result[i] = 0;
        }

        drop(data);
        compute.subdivision_buffer.unmap();
    } else {
        panic!("Failed to run get subdivision buffer!")
    }
}

pub fn process_unsubdivision(
    compute: &mut Compute,
    render: &mut Render,
    octree: &mut Octree,
    cpu_octree: &CpuOctree,
    blocks: &Vec<CpuOctree>,
) {
    let slice = compute.unsubdivision_buffer.slice(..);
    let future = slice.map_async(wgpu::MapMode::Read);

    render.device.poll(wgpu::Maintain::Wait);

    if let Ok(()) = pollster::block_on(future) {
        let mut data = slice.get_mapped_range_mut();
        let result: &mut [u32] = unsafe { reinterpret::reinterpret_mut_slice(&mut data) };

        // Reset atomic counter
        let len = (result[0] as usize).min(MAX_UNSUBDIVISIONS_PER_FRAME - 1);
        result[0] = 0;

        // if len > 0 {
        //     println!("Processing {} unsubdivisions", len);
        // }
        for i in 1..=len {
            let node_index = result[i] as usize;
            octree.unsubdivide(node_index);

            let mut value = 0;

            let pos = octree.positions[node_index];
            let (_, voxel_depth, _) = octree.find_voxel(pos, None);
            let (cpu_index, cpu_depth, cpu_pos) =
                cpu_octree.find_voxel(pos, Some(voxel_depth), None);

            let tnipt = cpu_octree.nodes[cpu_index];
            if tnipt.pointer < BLOCK_OFFSET {
                value = tnipt.value;
            } else if tnipt.pointer > BLOCK_OFFSET {
                let block_index = tnipt.pointer as usize - BLOCK_OFFSET as usize;
                let block = &blocks[block_index];

                if voxel_depth == cpu_depth {
                    value = tnipt.value;
                } else {
                    let voxel_size = 2.0 / (1 << (cpu_depth + 1)) as f32;
                    let block_pos = (pos - cpu_pos) / voxel_size;

                    let (block_index, _, _) =
                        block.find_voxel(block_pos, Some(voxel_depth - cpu_depth), None);

                    value = block.nodes[block_index].value;
                }
            }

            octree.nodes[node_index] = Voxel::from_value(value).to_value();

            result[i] = 0;
        }

        drop(data);
        compute.unsubdivision_buffer.unmap();
    } else {
        panic!("failed to run compute on gpu!")
    }
}
