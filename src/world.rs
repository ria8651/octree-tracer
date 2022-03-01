use super::*;

struct World {
    pub chunks: Vec<CpuOctree>,
}

impl World {
    pub fn generate_world(procedual: &Procedural, gpu: &Gpu, blocks: &Vec<CpuOctree>) -> Self {
        let mut world = World { chunks: Vec::new() };
        world.chunks.push(CpuOctree::new(0));
        for i in 0..8 {
            let chunk = procedual.generate_chunk(gpu, blocks);

            world.chunks[0].nodes[i].pointer = CHUNK_OFFSET + world.chunks.len() as u32;
            world.chunks[0].nodes[i].value = chunk.top_mip;

            world.chunks.push(chunk);
        }

        world.chunks[0].generate_mip_tree(Some(blocks));
        world
    }
}
