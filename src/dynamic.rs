// use super::*;

// pub fn process_subdivision(render: &mut Render, octree: &mut Octree, cpu_octree: &mut Octree) {
//     let slice = render.subdivision_buffer.slice(..);
//     let future = slice.map_async(wgpu::MapMode::Read);

//     render.device.poll(wgpu::Maintain::Wait);

//     if let Ok(()) = pollster::block_on(future) {
//         let mut data = slice.get_mapped_range_mut();
//         let result: &mut [u32] = unsafe { reinterpret::reinterpret_mut_slice(&mut data) };

//         // Reset atomic counter
//         let len = result[0] as usize;
//         result[0] = 0;

//         for i in 1..=len {
//             // Compute shader returns voxel to subdivide
//             let tnipt = octree.nodes[result[i] as usize];
//             if tnipt < VOXEL_OFFSET {
//                 // panic!("Node already subdivided!");
//                 println!("Node already subdivided!");
//                 continue;
//             }

//             println!("Subdividing node {}", result[i]);

//             // Subdivide
//             let voxel_index = tnipt - VOXEL_OFFSET;
//             let pos = octree.voxel_positions[voxel_index as usize];

//             let (voxel_index, voxel_depth, _, _) = octree.get_node(pos, None);
//             if voxel_depth < 20 {
//                 octree.subdivide(voxel_index, 0b10110111, true, voxel_depth + 1);
//             }

//             result[i] = 0;
//         }

//         drop(data);
//         render.subdivision_buffer.unmap();
//     } else {
//         panic!("Failed to run get subdivision buffer!")
//     }
// }
