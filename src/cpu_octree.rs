use super::*;

pub const CHUNK_OFFSET: u32 = 2147483648;

#[derive(Copy, Clone)]
pub struct Node {
    pub pointer: u32,
    pub value: Voxel,
}

impl Node {
    pub fn new(pointer: u32, value: Voxel) -> Self {
        Node { value, pointer }
    }
}

pub struct CpuOctree {
    pub nodes: Vec<Node>,
    pub top_mip: Voxel,
}

impl CpuOctree {
    pub fn new(mask: u8) -> Self {
        let mut octree = Self {
            top_mip: Voxel::new(50, 255, 50),
            nodes: Vec::new(),
        };
        octree.add_voxels(mask);
        octree
    }

    pub fn add_voxels(&mut self, mask: u8) {
        // Add 8 new voxels
        for i in 0..8 {
            if (mask >> i) & 1 != 0 {
                self.nodes.push(Node::new(
                    CHUNK_OFFSET + (self.nodes.len() as u32 % 8) + 1,
                    Voxel::new(255, 0, 0),
                ));
            } else {
                self.nodes
                    .push(Node::new(CHUNK_OFFSET, Voxel::new(0, 0, 0)));
            }
        }
    }

    // Returns (index, depth, pos)
    pub fn find_voxel(
        &self,
        pos: Vector3<f32>,
        max_depth: Option<u32>,
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

            if self.nodes[node_index + child_index].pointer >= CHUNK_OFFSET
                || depth == max_depth.unwrap_or(u32::MAX)
            {
                return (node_index + child_index, depth, node_pos);
            }

            node_index = self.nodes[node_index + child_index].pointer as usize;
        }
    }

    /// Takes a pointer to the first child NOT to the parent
    pub fn get_node_mask(&self, node: usize) -> [Voxel; 8] {
        let mut mask = [Voxel::new(0, 0, 0); 8];
        for i in 0..8 {
            mask[i] = self.nodes[node + i].value;
        }
        mask
    }

    pub fn put_in_block(&mut self, pos: Vector3<f32>, block_id: u32, depth: u32) {
        loop {
            let (node, node_depth, _) = self.find_voxel(pos, None);
            if depth == node_depth {
                self.nodes[node] = Node::new(CHUNK_OFFSET + block_id, Voxel::new(0, 0, 0));
                return;
            } else {
                self.nodes[node].pointer = self.nodes.len() as u32;
                self.add_voxels(0);
            }
        }
    }

    pub fn put_in_voxel(&mut self, pos: Vector3<f32>, voxel: Voxel, depth: u32) {
        loop {
            let (node, node_depth, _) = self.find_voxel(pos, None);
            if depth == node_depth {
                self.nodes[node] = Node::new(CHUNK_OFFSET, voxel);
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

        // println!("SVO size: {}", octree.nodes.len());
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
            if octree.nodes[node_index].pointer > CHUNK_OFFSET {
                if data_index < node_end {
                    let child_mask = data[data_start + data_index];
                    octree.nodes[node_index].pointer = octree.nodes.len() as u32;
                    octree.add_voxels(child_mask);
                }

                data_index += 1;
            }

            node_index += 1;
        }

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
                Voxel::new(colour[0], colour[1], colour[2]),
                depth as u32,
            );
        }

        return Ok(octree);
    }

    #[allow(dead_code)]
    pub fn load_structure(path: String) -> Vec<(Vector3<i32>, u32)> {
        let file = std::fs::read(path).unwrap();

        let vox_data = dot_vox::load_bytes(&file).unwrap();
        let size = vox_data.models[0].size;

        let mut voxels = Vec::new();
        for voxel in &vox_data.models[0].voxels {
            let pos = Vector3::new(
                size.x as i32 / 2 - voxel.x as i32,
                voxel.z as i32,
                voxel.y as i32 - size.y as i32 / 2,
            );
            voxels.push((pos, voxel.i as u32 + 1));
        }

        return voxels;
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
            if node.pointer < CHUNK_OFFSET {
                octree
                    .nodes
                    .push(octree::create_node(node.pointer as usize));
            } else {
                octree.nodes.push(node.value.to_value());
            }
        }

        octree
    }

    pub fn raw(&self) -> Vec<u32> {
        let mut raw = Vec::new();
        for node in &self.nodes {
            raw.push(node.pointer);
        }
        raw
    }

    pub unsafe fn bin(&self) -> &[u8] {
        reinterpret::reinterpret_slice(&self.nodes)
    }

    pub unsafe fn from_bin(bin: Vec<u8>) -> CpuOctree {
        let nodes = reinterpret::reinterpret_vec(bin);
        CpuOctree {
            nodes,
            top_mip: Voxel::new(0, 0, 0),
        }
    }
}

impl std::fmt::Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let voxel = self.value;
        if self.pointer < CHUNK_OFFSET {
            write!(
                f,
                "{:25} Pointer: {}",
                format!("  Voxel: ({}, {}, {})", voxel.r, voxel.g, voxel.b),
                self.pointer
            )
        } else {
            write!(
                f,
                "{:25} Pointer: BlockID: {}",
                format!("  Voxel: ({}, {}, {})", voxel.r, voxel.g, voxel.b),
                self.pointer - CHUNK_OFFSET
            )
        }
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
