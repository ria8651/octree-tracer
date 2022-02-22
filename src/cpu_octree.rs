use super::*;

pub const BLOCK_OFFSET: u32 = 2147483648;

#[derive(Copy, Clone)]
pub struct Node {
    pub pointer: u32,
    pub value: u32,
}

impl Node {
    fn new(pointer: u32, value: u32) -> Self {
        Node { value, pointer }
    }
}

pub struct CpuOctree {
    pub nodes: Vec<Node>,
}

impl CpuOctree {
    pub fn new(mask: u8) -> Self {
        let mut octree = Self { nodes: Vec::new() };
        octree.add_voxels(mask);
        octree
    }

    pub fn add_voxels(&mut self, mask: u8) {
        // Add 8 new voxels
        for i in 0..8 {
            if (mask >> i) & 1 != 0 {
                self.nodes.push(Node::new(
                    BLOCK_OFFSET + 1 + (self.nodes.len() as u32 % 3),
                    create_voxel(
                        ((mask & 0b10111001) % 4) * 85 + 1,
                        ((mask & 0b00101001) % 7) * 42 + 1,
                        ((mask & 0b10100101) % 3) * 128 + 1,
                    ),
                ));
            } else {
                self.nodes.push(Node::new(BLOCK_OFFSET, 0));
            }
        }
    }

    /// Returns (index, depth, pos)
    pub fn find_voxel(
        &self,
        pos: Vector3<f32>,
        max_depth: Option<u32>,
        blocks: Option<Vec<CpuOctree>>,
    ) -> (usize, u32, Vector3<f32>) {
        let mut node_index = 0;
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

            if self.nodes[node_index + child_index].pointer >= BLOCK_OFFSET
                || depth == max_depth.unwrap_or(u32::MAX)
            {
                return (node_index + child_index, depth, node_pos);
            }

            node_index = self.nodes[node_index + child_index].pointer as usize;
            // let p = node_index + child_index;
            // if depth == max_depth.unwrap_or(u32::MAX) {
            //     return (p, depth, node_pos);
            // } else if self.nodes[p].pointer >= BLOCK_OFFSET {
            //     if let Some(blocks) = blocks {
            //         let block = &blocks[(self.nodes[p].pointer - BLOCK_OFFSET) as usize];
            //         return block.find_voxel(pos, max_depth, None);
            //     } else {
            //         return (p, depth, node_pos);
            //     }
            // }

            // node_index = self.nodes[p].pointer as usize;
        }
    }

    /// Takes a pointer to the first child NOT to the parent
    pub fn get_node_mask(&self, node: usize) -> [Voxel; 8] {
        let mut mask = [Voxel::new(0, 0, 0); 8];
        for i in 0..8 {
            mask[i] = Voxel::from_value(self.nodes[node + i].value);
        }
        mask
    }

    pub fn put_in_voxel(&mut self, pos: Vector3<f32>, value: u32, depth: u32) {
        loop {
            let (node, node_depth, _) = self.find_voxel(pos, None, None);
            if depth == node_depth {
                self.nodes[node] = Node::new(BLOCK_OFFSET + 1, value);
                return;
            } else {
                self.nodes[node].pointer = self.nodes.len() as u32;
                self.add_voxels(0);
            }
        }
    }

    pub fn load_file(file: String, octree_depth: u32) -> Result<CpuOctree, String> {
        let path = std::path::Path::new(&file);
        let data = std::fs::read(path).map_err(|e| e.to_string())?;
        use std::ffi::OsStr;
        let octree = match path.extension().and_then(OsStr::to_str) {
            Some("rsvo") => CpuOctree::load_octree(&data, octree_depth)?,
            Some("vox") => CpuOctree::load_vox(&data)?,
            _ => return Err("Unknown file type".to_string()),
        };

        // println!("{:?}", octree);
        // panic!();

        return Ok(octree);
    }

    // Models from https://github.com/ephtracy/voxel-model/tree/master/svo
    fn load_octree(data: &[u8], octree_depth: u32) -> Result<CpuOctree, String> {
        let top_level_start = 16;
        let node_count_start = 20;

        let top_level = data[top_level_start] as usize;

        let data_start = node_count_start + 4 * (top_level + 1);

        let mut node_counts = Vec::new();
        for i in 0..(top_level + 1) {
            let node_count = u32::from_be_bytes([
                data[node_count_start + i * 4 + 3],
                data[node_count_start + i * 4 + 2],
                data[node_count_start + i * 4 + 1],
                data[node_count_start + i * 4],
            ]);

            node_counts.push(node_count);
        }

        if octree_depth as usize > top_level {
            return Err(format!(
                "Octree depth ({}) is greater than top level ({})",
                octree_depth, top_level
            ));
        }

        let node_end = node_counts[0..octree_depth as usize].iter().sum::<u32>() as usize;

        let mut octree = CpuOctree::new(data[data_start]);
        let mut data_index = 1;
        let mut node_index = 0;
        while node_index < octree.nodes.len() {
            if octree.nodes[node_index].pointer > BLOCK_OFFSET {
                if data_index < node_end {
                    let child_mask = data[data_start + data_index];
                    octree.nodes[node_index].pointer = octree.nodes.len() as u32;
                    octree.add_voxels(child_mask);
                }
                // else {
                //     octree.nodes[node_index] = Node::new(create_voxel(255, 50, 50), 0);
                // }

                data_index += 1;
            }

            node_index += 1;
        }

        octree.generate_mip_tree();

        println!("SVO size: {}", octree.nodes.len());
        Ok(octree)
    }

    fn load_vox(file: &[u8]) -> Result<CpuOctree, String> {
        let vox_data = dot_vox::load_bytes(file)?;
        let size = vox_data.models[0].size;
        if size.x != size.y || size.x != size.z || size.y != size.z {
            return Err("Voxel model is not a cube!".to_string());
        }

        let size = size.x as i32;

        let depth = (size as f32).log2();
        if depth != depth.floor() {
            return Err("Voxel model size is not a power of 2!".to_string());
        }

        let mut octree = CpuOctree::new(0);
        for voxel in &vox_data.models[0].voxels {
            let colour = vox_data.palette[voxel.i as usize].to_le_bytes();
            let mut pos = Vector3::new(
                size as f32 - voxel.x as f32 - 1.0,
                voxel.z as f32,
                voxel.y as f32,
            );
            pos /= size as f32;
            pos = pos * 2.0 - Vector3::new(1.0, 1.0, 1.0);

            octree.put_in_voxel(
                pos,
                Voxel::new(colour[0], colour[1], colour[2]).to_cpu_value(),
                depth as u32,
            );
        }

        octree.generate_mip_tree();

        println!("SVO size: {}", octree.nodes.len());
        return Ok(octree);
    }

    // This function assumes that the bottem level is filled with colours and overides all other colours
    pub fn generate_mip_tree(&mut self) {
        let mut voxels_in_each_level: Vec<Vec<usize>> = Vec::new();

        use std::collections::VecDeque;
        let mut queue = VecDeque::new();

        for child_index in 0..8 {
            let node = self.nodes[child_index];
            if node.pointer < BLOCK_OFFSET {
                queue.push_back((child_index, 1));
            }
        }

        while let Some((node_index, depth)) = queue.pop_front() {
            loop {
                if let Some(level) = voxels_in_each_level.get_mut(depth as usize) {
                    level.push(node_index);

                    let node = self.nodes[node_index as usize];
                    for child_index in 0..8 {
                        let child_node = self.nodes[node.pointer as usize + child_index];
                        if child_node.pointer < BLOCK_OFFSET {
                            queue.push_back((node.pointer as usize + child_index, depth + 1));
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
        //     // for j in 0..voxels_in_each_level[i].len() {
        //     //     println!("  {}", voxels_in_each_level[i][j]);
        //     // }
        // }

        for i in (1..voxels_in_each_level.len()).rev() {
            for node_index in &voxels_in_each_level[i] {
                // Average the colours of the 8 children
                let node = self.nodes[*node_index as usize];
                let mut colour = Vector3::new(0.0, 0.0, 0.0);
                let mut divisor = 0.0;

                for i in 0..8 {
                    let child = self.nodes[node.pointer as usize + i];
                    if child.pointer != BLOCK_OFFSET {
                        let voxel = Voxel::from_value(child.value);
                        colour += Vector3::new(voxel.r as f32, voxel.g as f32, voxel.b as f32);
                        divisor += 1.0;
                    }
                }

                colour /= divisor;

                self.nodes[*node_index as usize].value =
                    Voxel::new(colour.x as u8, colour.y as u8, colour.z as u8)
                        .to_cpu_value()
                        .max(1);
            }
        }

        // println!("{:?}", self);
        // panic!();
    }

    #[allow(dead_code)]
    pub fn to_octree(&self) -> Octree {
        let mut octree = Octree {
            nodes: Vec::new(),
            positions: Vec::new(),
            hole_stack: Vec::new(),
        };

        for i in 0..self.nodes.len() {
            let node = self.nodes[i];
            if node.pointer > 0 {
                octree
                    .nodes
                    .push(octree::create_node(node.pointer as usize));
            } else {
                octree.nodes.push(Voxel::from_value(node.value).to_value());
            }
        }

        octree
    }
}

fn create_voxel(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) << 16 | (g as u32) << 8 | b as u32
}

impl std::fmt::Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let voxel = Voxel::from_value(self.value);
        write!(
            f,
            "  Voxel: ({}, {}, {}), Pointer: {}",
            voxel.r, voxel.g, voxel.b, self.pointer
        )
    }
}

impl std::fmt::Debug for CpuOctree {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Nodes ({}):\n", self.nodes.len())?;
        let mut c = 0;
        for value in &self.nodes {
            write!(f, "{:?}\n", *value)?;

            c += 1;
            if c % 8 == 0 {
                write!(f, "\n")?;
            }
        }

        Ok(())
    }
}
