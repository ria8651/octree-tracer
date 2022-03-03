use super::*;

pub struct World {
    pub chunks: HashMap<u32, CpuOctree>,
}

impl World {
    pub fn new() -> Self {
        let mut world = Self {
            chunks: HashMap::new(),
        };

        world.chunks.insert(
            1,
            CpuOctree::load_file("blocks/stone.vox".to_string(), 0, None).unwrap(),
        );
        world.chunks.insert(
            2,
            CpuOctree::load_file("blocks/dirt.vox".to_string(), 0, None).unwrap(),
        );
        world.chunks.insert(
            3,
            CpuOctree::load_file("blocks/grass.vox".to_string(), 0, None).unwrap(),
        );
        world.chunks.insert(
            4,
            CpuOctree::load_file("blocks/wood.vox".to_string(), 0, None).unwrap(),
        );
        world.chunks.insert(
            5,
            CpuOctree::load_file("blocks/leaf.vox".to_string(), 0, None).unwrap(),
        );
        world.chunks.insert(
            6,
            CpuOctree::load_file("blocks/slate.vox".to_string(), 0, None).unwrap(),
        );
        world.chunks.insert(
            7,
            CpuOctree::load_file("blocks/crystal.vox".to_string(), 0, None).unwrap(),
        );
        world.chunks.insert(
            8,
            CpuOctree::load_file("blocks/glass.vox".to_string(), 0, None).unwrap(),
        );
        world
    }

    // pub fn generate_world(
    //     &self,
    //     procedual: &Procedural,
    //     gpu: &Gpu,
    //     blocks: &Vec<CpuOctree>,
    // ) -> Self {
    //     let mut world = World::new();

    //     world.chunks.insert(
    //         0,
    //         CpuOctree::load_file("files/dragon.rsvo".to_string(), 12, Some(&world)).unwrap(),
    //     );

    //     world.chunks.push(CpuOctree::new(0));
    //     for i in 0..8 {
    //         let chunk = procedual.generate_chunk(gpu, blocks);

    //         world.chunks[0].nodes[i].pointer = CHUNK_OFFSET + world.chunks.len() as u32;
    //         world.chunks[0].nodes[i].value = chunk.top_mip;

    //         world.chunks.push(chunk);
    //     }

    //     world.chunks[0].generate_mip_tree(Some(blocks));

    //     world
    // }

    /// Returns (chunk, index, depth, pos)
    pub fn find_voxel(
        &self,
        pos: Vector3<f32>,
        max_depth: Option<u32>,
    ) -> (u32, usize, u32, Vector3<f32>) {
        let mut node_index = 0;
        let mut chunk = 0;
        let mut node_pos = Vector3::zero();
        let mut depth = 0;
        loop {
            depth += 1;

            let p = Vector3::new(
                (pos.x >= node_pos.x) as usize,
                (pos.y >= node_pos.y) as usize,
                (pos.z >= node_pos.z) as usize,
            );
            let child_index = p.x * 4 + p.y * 2 + p.z;

            node_pos += Octree::pos_offset(child_index, depth);

            let tnipt = self.chunks[&chunk].nodes[node_index + child_index].pointer;
            if tnipt == CHUNK_OFFSET || depth == max_depth.unwrap_or(u32::MAX) {
                return (chunk, node_index + child_index, depth, node_pos);
            } else if tnipt > CHUNK_OFFSET {
                chunk = tnipt - CHUNK_OFFSET;
                node_index = 0;
            } else {
                node_index = tnipt as usize;
            }
        }
    }
}
