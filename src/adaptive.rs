use super::*;

pub const MAX_SUBDIVISIONS_PER_FRAME: usize = 1024000;
pub const MAX_UNSUBDIVISIONS_PER_FRAME: usize = 1024000;

pub fn process_subdivision(compute: &mut Compute, gpu: &Gpu, octree: &mut Octree, world: &World) {
    let slice = compute.subdivision_buffer.slice(..);
    let future = slice.map_async(wgpu::MapMode::Read);

    gpu.device.poll(wgpu::Maintain::Wait);

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
            let (cpu_chunk, cpu_index, _, _) = world.find_voxel(pos, Some(voxel_depth));

            let tnipt = world.chunks[&cpu_chunk].nodes[cpu_index];
            if tnipt.pointer < CHUNK_OFFSET {
                let mask = world.chunks[&cpu_chunk].get_node_mask(tnipt.pointer as usize);
                octree.subdivide(node_index, mask, voxel_depth + 1);
            } else if tnipt.pointer > CHUNK_OFFSET {
                let chunk = tnipt.pointer - CHUNK_OFFSET;
                let mask = world.chunks[&chunk].get_node_mask(0);
                octree.subdivide(node_index, mask, voxel_depth + 1);
            }

            result[i] = 0;
        }

        drop(data);
        compute.subdivision_buffer.unmap();
    } else {
        panic!("Failed to run get subdivision buffer!")
    }
}

pub fn process_unsubdivision(compute: &mut Compute, gpu: &Gpu, octree: &mut Octree, world: &World) {
    let slice = compute.unsubdivision_buffer.slice(..);
    let future = slice.map_async(wgpu::MapMode::Read);

    gpu.device.poll(wgpu::Maintain::Wait);

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

            let pos = octree.positions[node_index];
            let (_, voxel_depth, _) = octree.find_voxel(pos, None);
            let (cpu_chunk, cpu_index, _, _) = world.find_voxel(pos, Some(voxel_depth));

            let tnipt = world.chunks[&cpu_chunk].nodes[cpu_index];
            let value = if tnipt.pointer < CHUNK_OFFSET {
                tnipt.value
            } else if tnipt.pointer > CHUNK_OFFSET {
                let chunk = tnipt.pointer - CHUNK_OFFSET;
                world.chunks[&chunk].top_mip
            } else {
                tnipt.value
            };

            octree.nodes[node_index] = value.to_value();

            result[i] = 0;
        }

        drop(data);
        compute.unsubdivision_buffer.unmap();
    } else {
        panic!("failed to run compute on gpu!")
    }
}
