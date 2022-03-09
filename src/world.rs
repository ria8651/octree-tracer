use super::*;

pub struct World {
    pub path: String,
    pub chunks: HashMap<u32, CpuOctree>,
}

impl World {
    pub fn new(path: String) -> Self {
        let mut world = Self {
            path,
            chunks: HashMap::new(),
        };

        world.chunks.insert(
            1,
            CpuOctree::load_file("blocks/stone.vox".to_string(), 0).unwrap(),
        );
        world.generate_mip_tree(1);
        world.chunks.insert(
            2,
            CpuOctree::load_file("blocks/dirt.vox".to_string(), 0).unwrap(),
        );
        world.generate_mip_tree(2);
        world.chunks.insert(
            3,
            CpuOctree::load_file("blocks/grass.vox".to_string(), 0).unwrap(),
        );
        world.generate_mip_tree(3);
        world.chunks.insert(
            4,
            CpuOctree::load_file("blocks/wood.vox".to_string(), 0).unwrap(),
        );
        world.generate_mip_tree(4);
        world.chunks.insert(
            5,
            CpuOctree::load_file("blocks/leaf.vox".to_string(), 0).unwrap(),
        );
        world.generate_mip_tree(5);
        world.chunks.insert(
            6,
            CpuOctree::load_file("blocks/slate.vox".to_string(), 0).unwrap(),
        );
        world.generate_mip_tree(6);
        world.chunks.insert(
            7,
            CpuOctree::load_file("blocks/crystal.vox".to_string(), 0).unwrap(),
        );
        world.generate_mip_tree(7);
        world.chunks.insert(
            8,
            CpuOctree::load_file("blocks/glass.vox".to_string(), 0).unwrap(),
        );
        world.generate_mip_tree(8);

        world
    }

    pub fn generate_world(procedual: &mut Procedural, gpu: &Gpu) -> Self {
        let mut world = World::new("".to_string());

        // let root = procedual.generate_chunk(gpu, Vector3::new(-1.0, -1.0, -1.0), 0);

        let mut root = CpuOctree::new(0);
        let world_depth = 2;

        let world_size = 1 << world_depth;
        let voxel_size = 2.0 / world_size as f32;
        let total_iterations = world_size * world_size * world_size;

        use indicatif::{ProgressBar, ProgressStyle};
        let pb = ProgressBar::new(total_iterations);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] [{wide_bar:.green/blue}] {pos}/{len} chunks generated ({eta_precise})")
                .progress_chars("=> "),
        );
        pb.set_position(0);

        let mut i = 0;
        for x in 0..world_size {
            for y in 0..world_size {
                for z in 0..world_size {
                    let pos = (Vector3::new(x as f32, y as f32, z as f32)) * voxel_size
                        - Vector3::new(1.0, 1.0, 1.0);

                    let index = CHUNK_OFFSET / 2 + i as u32;
                    let chunk = procedual.generate_chunk(gpu, pos, world_depth);
                    world.chunks.insert(index, chunk);
                    world.generate_mip_tree(index);

                    root.put_in_block(pos, index, world_depth);

                    i += 1;
                    pb.set_position(i);
                }
            }
        }

        println!();

        world.chunks.insert(0, root);
        world.generate_mip_tree(0);

        world
    }

    pub fn save_world<S: AsRef<std::ffi::OsStr> + Sized>(&self, path: S) -> Result<(), String> {
        // Write chunk to file
        use std::io::Write;
        let path = std::path::Path::new(&path);
        if path.exists() {
            return Err("File already exists".to_string());
        }

        std::fs::create_dir(path).unwrap();
        for (index, chunk) in &self.chunks {
            if *index == 0 || *index >= CHUNK_OFFSET / 2 {
                let mut file =
                    std::fs::File::create(path.join(index.to_string() + ".bin")).unwrap();
                let data = unsafe { chunk.bin() };
                file.write_all(data).unwrap();
            }
        }

        Ok(())
    }

    pub fn load_world<S: AsRef<std::ffi::OsStr> + Sized>(path: S) -> Result<Self, String> {
        let path = std::path::Path::new(&path);
        let mut world = World::new(path.to_str().unwrap().to_string());
        if !path.exists() {
            return Err("File doesn't exist!".to_string());
        }

        let file = std::fs::read(path.join("0.bin")).unwrap();
        let root = unsafe { CpuOctree::from_bin(file) };
        world.chunks.insert(0, root);

        Ok(world)
    }

    pub fn load_chunk(&mut self, index: u32) {
        let path = self.path.clone() + "/" + &index.to_string() + ".bin";
        let file = std::fs::read(path).unwrap();
        let root = unsafe { CpuOctree::from_bin(file) };
        self.chunks.insert(index, root);
    }

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

    pub fn generate_mip_tree(&mut self, id: u32) {
        // let chunk = self
        //     .chunks
        //     .get_mut(&chunk_id)
        //     .expect("Tried to generate mip tree for chunk that doesn't exist");

        let mut voxels_in_each_level: Vec<Vec<usize>> = Vec::new();
        voxels_in_each_level.push(vec![0]);

        use std::collections::VecDeque;
        let mut queue = VecDeque::new();

        for child_index in 0..8 {
            let node = self.chunks[&id].nodes[child_index];
            if node.pointer < CHUNK_OFFSET {
                queue.push_back((child_index, 1));
            } else if node.pointer > CHUNK_OFFSET {
                let index = node.pointer - CHUNK_OFFSET;
                self.chunks.get_mut(&id).unwrap().nodes[child_index].value =
                    self.chunks[&index].top_mip;
            }
        }

        while let Some((node_index, depth)) = queue.pop_front() {
            loop {
                if let Some(level) = voxels_in_each_level.get_mut(depth as usize) {
                    level.push(node_index);

                    let node = self.chunks[&id].nodes[node_index as usize];
                    for child_index in 0..8 {
                        let child_node =
                            self.chunks[&id].nodes[node.pointer as usize + child_index];
                        if child_node.pointer < CHUNK_OFFSET {
                            queue.push_back((node.pointer as usize + child_index, depth + 1));
                        } else if child_node.pointer > CHUNK_OFFSET {
                            let index = child_node.pointer - CHUNK_OFFSET;
                            self.chunks.get_mut(&id).unwrap().nodes
                                [node.pointer as usize + child_index]
                                .value = self.chunks[&index].top_mip;
                        }
                    }

                    break;
                } else {
                    voxels_in_each_level.push(Vec::new());
                }
            }
        }

        // for i in 0..voxels_in_each_level.len() {
        //     println!("Level {}: ({})", i, voxels_in_each_level[i].len());
        //     for j in 0..voxels_in_each_level[i].len() {
        //         println!("  {}", voxels_in_each_level[i][j]);
        //     }
        // }

        for i in (0..voxels_in_each_level.len()).rev() {
            for node_index in &voxels_in_each_level[i] {
                // Average the colours of the 8 children
                let node = if i != 0 {
                    self.chunks[&id].nodes[*node_index as usize]
                } else {
                    Node::new(0, Voxel::new(0, 0, 0))
                };
                let mut colour = Vector3::new(0.0, 0.0, 0.0);
                let mut divisor = 0.0;

                for i in 0..8 {
                    let child = self.chunks[&id].nodes[node.pointer as usize + i];
                    if child.value != Voxel::new(0, 0, 0) {
                        let voxel = child.value;
                        colour += Vector3::new(voxel.r as f32, voxel.g as f32, voxel.b as f32);
                        divisor += 1.0;
                    }
                }

                colour /= divisor;

                let voxel = Voxel::new(
                    (colour.x as u8).max(1),
                    (colour.y as u8).max(1),
                    (colour.z as u8).max(1),
                );

                if i != 0 {
                    self.chunks.get_mut(&id).unwrap().nodes[*node_index as usize].value = voxel;
                } else {
                    self.chunks.get_mut(&id).unwrap().top_mip = voxel;
                }
            }
        }

        // println!("{:?}", self);
        // panic!();
    }
}
