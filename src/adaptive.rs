use super::*;

// TODO: There isn't 256 million pixels, why do lower values crash?
pub const MAX_SUBDIVISIONS_PER_FRAME: usize = 1024000;
pub const MAX_UNSUBDIVISIONS_PER_FRAME: usize = 1024000;

pub fn process_subdivision(
    compute: &mut Compute,
    render: &mut Render,
    octree: &mut Octree,
    cpu_octree: &Octree,
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

        if len > 0 {
            println!("Processing {} subdivisions", len);
        }
        for i in 1..=len {
            let node_index = result[i] as usize;

            if octree.get_node(node_index) < VOXEL_OFFSET {
                // println!("Doubleup!");
                continue;
            }

            let pos = octree.positions[node_index];
            let (voxel_index, voxel_depth, voxel_pos) = octree.find_voxel(pos, None);
            let (cpu_index, _, _) = cpu_octree.find_voxel(pos, Some(voxel_depth));

            let tnipt = cpu_octree.get_node(cpu_index);
            if tnipt < VOXEL_OFFSET {
                let mask = cpu_octree.get_node_mask(tnipt as usize);
                octree.subdivide(node_index, mask, voxel_depth + 1);
            }
            // else {
            //     panic!("Tried to subdivide bottom level voxel!");
            // }

            if voxel_index != node_index || voxel_pos != pos {
                panic!("Incorrect voxel position!");
            }

            result[i] = 0;
        }

        drop(data);
        compute.subdivision_buffer.unmap();
    } else {
        panic!("Failed to run get subdivision buffer!")
    }
}

pub fn process_unsubdivision(compute: &mut Compute, render: &mut Render, octree: &mut Octree) {
    let slice = compute.unsubdivision_buffer.slice(..);
    let future = slice.map_async(wgpu::MapMode::Read);

    render.device.poll(wgpu::Maintain::Wait);

    if let Ok(()) = pollster::block_on(future) {
        let mut data = slice.get_mapped_range_mut();
        let result: &mut [u32] = unsafe { reinterpret::reinterpret_mut_slice(&mut data) };

        // Reset atomic counter
        let len = (result[0] as usize).min(MAX_UNSUBDIVISIONS_PER_FRAME - 1);
        result[0] = 0;

        if len > 0 {
            println!("Processing {} unsubdivisions", len);
        }
        for i in 1..=len {
            octree.unsubdivide(result[i] as usize);

            result[i] = 0;
        }

        drop(data);
        compute.unsubdivision_buffer.unmap();
    } else {
        panic!("failed to run compute on gpu!")
    }
}
