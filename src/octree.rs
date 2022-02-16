use cgmath::*;

// First palette colour is empty voxel
// const PALETTE: [u32; 3] = [0x00000000, 0x0000FF00, 0x000000FF];
pub const VOXEL_OFFSET: u32 = u32::MAX / 2;

// Layout
// 01100101 01100101 01100101 01100101
//  ^---- Node: Pointer to children, Voxel: Palette index
// ^----- 0: Node, 1: Voxel
pub struct Octree {
    pub nodes: Vec<u32>,
    pub voxels: Vec<u32>,
    // stays on cpu
    pub voxel_positions: Vec<Vector3<f32>>,
}

impl Octree {
    pub fn new(mask: u8) -> Self {
        let nodes = Vec::new();
        let mut voxels = Vec::new();
        let voxel_positions = Vec::new();
        // le empty voxel
        voxels.push(0);

        let mut octree = Self { nodes, voxels, voxel_positions };
        octree.add_voxels(mask, true);
        octree
    }

    pub fn create_voxel(&mut self, value: u32) -> u32 {
        let voxel_index = VOXEL_OFFSET + self.voxels.len() as u32;
        self.voxels.push(value);
        voxel_index
    }

    pub fn add_voxels(&mut self, mask: u8, bottom_level: bool) {
        // Add 8 new voxels
        for i in 0..8 {
            if mask >> i & 1 != 0 {
                if bottom_level {
                    let voxel = self.create_voxel(1);
                    self.nodes.push(voxel);
                } else {
                    self.nodes.push(u32::MAX);
                }
            } else {
                self.nodes.push(VOXEL_OFFSET);
            }
        }
    }

    pub fn subdivide(&mut self, node: usize, mask: u8, bottom_level: bool) {
        if self.nodes[node] < VOXEL_OFFSET {
            panic!("Node already subdivided!");
        }

        if bottom_level == true {
            let voxel_index = self.nodes[node] - VOXEL_OFFSET;
            self.voxels[voxel_index as usize] = 0;
        }

        // Turn voxel into node
        self.nodes[node] = self.nodes.len() as u32;

        self.add_voxels(mask, bottom_level);
    }

    pub fn put_in_voxel(&mut self, pos: Vector3<f32>, value: u32, depth: u32) {
        loop {
            let (node, node_depth, _) = self.get_node(pos);
            if depth == node_depth {
                self.nodes[node] = self.create_voxel(value);
                self.voxels.push(value);
                return;
            } else {
                self.subdivide(node, 0x00000000, true);
            }
        }
    }

    pub fn get_node(&self, pos: Vector3<f32>) -> (usize, u32, Vector3<f32>) {
        let mut node_index = 0;
        let mut node_pos = Vector3::new(0.0, 0.0, 0.0);
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

            if self.nodes[node_index + child_index] >= VOXEL_OFFSET {
                return (node_index + child_index, depth, node_pos);
            }

            node_index = self.nodes[node_index + child_index] as usize;
        }
    }

    pub fn fill_voxel_positions(&mut self) {
        self.voxel_positions = vec![Vector3::zero(); self.voxels.len()];

        let mut stack = Vec::new();
        for child_index in 0..8 {
            let child_depth = 1;
            let child_pos = Octree::pos_offset(child_index, child_depth);
            println!("{:?}", child_pos);
            stack.push((child_index, child_depth, child_pos));
        }
        while let Some((node_index, depth, pos)) = stack.pop() {
            let tnipt = self.nodes[node_index as usize];
            if tnipt >= VOXEL_OFFSET {
                let voxel_index = tnipt - VOXEL_OFFSET;
                if voxel_index == 0 {
                    continue;
                }
                self.voxel_positions[voxel_index as usize] = pos;
            } else {
                for child_index in 0..8 {
                    let new_index = tnipt as usize + child_index;
                    let new_depth = depth + 1;
                    let new_pos = pos + Octree::pos_offset(child_index, new_depth);
                    stack.push((new_index, new_depth, new_pos));
                }
            }
        }
    }

    pub fn expand(&mut self, size: usize) {
        self.nodes
            .extend(std::iter::repeat(0).take(size - self.nodes.len()));
        self.voxels
            .extend(std::iter::repeat(0).take(size - self.voxels.len()));
    }

    pub fn raw_data(&self) -> (&Vec<u32>, &Vec<u32>) {
        (&self.nodes, &self.voxels)
    }

    pub fn node_len(&self) -> usize {
        self.nodes.len()
    }

    pub fn voxel_len(&self) -> usize {
        self.voxels.len()
    }

    fn pos_offset(child_index: usize, depth: u32) -> Vector3<f32> {
        let x = (child_index >> 2) & 1;
        let y = (child_index >> 1) & 1;
        let z = child_index & 1;

        (Vector3::new(x as f32, y as f32, z as f32) * 2.0 - Vector3::new(1.0, 1.0, 1.0))
            / (1 << depth) as f32
    }
}

// fn count_bits(mut n: u8) -> usize {
//     let mut count = 0;
//     while n != 0 {
//         n = n & (n - 1);
//         count += 1;
//     }
//     return count;
// }

impl std::fmt::Debug for Octree {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Nodes ({}):\n", self.nodes.len())?;
        let mut c = 0;
        for value in &self.nodes {
            if *value >= VOXEL_OFFSET {
                let voxel_index = *value - VOXEL_OFFSET;
                // let stringgy;
                // if voxel_index == 0 {
                //     stringgy = "empty".to_string();
                // } else {
                //     let col = PALETTE[self.voxels[voxel_index as usize] as usize];
                //     stringgy =
                //         format!("{}, {}, {}", (col >> 16) as u8, (col >> 8) as u8, col as u8);
                // }

                write!(f, "  Leaf: {}\n", voxel_index)?;
            } else {
                write!(f, "  Node: {}\n", value)?;
            }

            c += 1;
            if c % 8 == 0 {
                write!(f, "\n")?;
            }
        }

        write!(f, "\nVoxels ({}):\n", self.voxels.len())?;
        for value in &self.voxels {
            write!(f, "  Voxel: {}\n", value)?;
        }

        Ok(())
    }
}

pub fn load_file(file: String, octree_depth: u32) -> Result<Octree, String> {
    let path = std::path::Path::new(&file);
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    use std::ffi::OsStr;
    let octree = match path.extension().and_then(OsStr::to_str) {
        Some("rsvo") => load_octree(&data, octree_depth),
        Some("vox") => load_vox(&data),
        _ => Err("Unknown file type".to_string()),
    }?;

    // println!("{:?}", octree);
    // panic!();

    return Ok(octree);
}

// Models from https://github.com/ephtracy/voxel-model/tree/master/svo
fn load_octree(data: &[u8], octree_depth: u32) -> Result<Octree, String> {
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

    let node_end = node_counts[0..octree_depth as usize].iter().sum::<u32>() as usize;

    let mut octree = Octree::new(data[data_start]);

    let mut data_index = 1;
    let mut node_index = 0;
    while node_index < octree.nodes.len() {
        if octree.nodes[node_index] != VOXEL_OFFSET {
            if data_index < node_end {
                let child_mask = data[data_start + data_index];
                octree.subdivide(node_index, child_mask, false);
            } else {
                octree.nodes[node_index] = octree.create_voxel(1);
            }

            data_index += 1;
        }

        node_index += 1;
    }

    println!("SVO size: {}", octree.nodes.len());
    Ok(octree)
}

fn load_vox(file: &[u8]) -> Result<Octree, String> {
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

    let mut octree = Octree::new(0x00000000);
    for voxel in &vox_data.models[0].voxels {
        // let colour = vox_data.palette[voxel.i as usize].to_le_bytes();
        let mut pos = Vector3::new(
            size as f32 - voxel.x as f32 - 1.0,
            voxel.z as f32,
            voxel.y as f32,
        );
        pos /= size as f32;
        pos = pos * 2.0 - Vector3::new(1.0, 1.0, 1.0);

        octree.put_in_voxel(pos, 1, depth as u32);
    }

    println!("SVO size: {}", octree.nodes.len());
    return Ok(octree);
}
