use cgmath::*;

// First palette colour is empty voxel
// const PALETTE: [u32; 3] = [0x00000000, 0x0000FF00, 0x000000FF];
pub const VOXEL_OFFSET: u32 = 134217728;

/// Layout (Outdated)
/// ```
/// 01100101 01100101 01100101 01100101
///  ^---- Node: pointer to children, Voxel: palette index
/// ^----- 0: Node, 1: Voxel
/// ```
pub struct Octree {
    pub nodes: Vec<u32>,
    // stays on cpu
    pub positions: Vec<Vector3<f32>>,
    pub hole_stack: Vec<usize>,
}

impl Octree {
    pub fn new(mask: u8) -> Self {
        let nodes = Vec::new();
        let positions = Vec::new();
        let hole_stack = Vec::new();

        let mut octree = Self {
            nodes,
            positions,
            hole_stack,
        };

        octree.add_voxels(mask, Vector3::zero(), 1);
        octree
    }

    pub fn get_node(&self, index: usize) -> u32 {
        self.nodes[index] >> 4
    }

    pub fn add_voxels(&mut self, mask: u8, voxel_pos: Vector3<f32>, depth: u32) {
        // Add 8 new voxels
        for i in 0..8 {
            let new_pos = voxel_pos + Octree::pos_offset(i, depth);
            if mask >> i & 1 != 0 {
                self.nodes.push(create_voxel(255, 50, 50));
            } else {
                self.nodes.push(create_voxel(0, 0, 0));
            }
            self.positions.push(new_pos);
        }
    }

    pub fn subdivide(&mut self, node: usize, mask: u8, depth: u32) {
        if self.get_node(node) < VOXEL_OFFSET {
            panic!("Node already subdivided!");
        }

        let pos = self.positions[node];
        if let Some(index) = self.hole_stack.pop() {
            self.nodes[node] = create_node(index);

            for i in 0..8 {
                let new_pos = pos + Octree::pos_offset(i, depth);
                let child_index = index + i;
                if mask >> i & 1 != 0 {
                    self.nodes[child_index] = create_voxel(50, 255, 50);
                } else {
                    self.nodes[child_index] = create_voxel(0, 0, 0);
                }
                self.positions[child_index] = new_pos;
            }
        } else {
            self.nodes[node] = create_node(self.nodes.len());

            self.add_voxels(mask, pos, depth);
        };
    }

    pub fn unsubdivide(&mut self, node: usize) {
        let tnipt = self.get_node(node);
        if tnipt >= VOXEL_OFFSET {
            println!("Node {} not subdivided!", node);
            return;
        }

        self.hole_stack.push(tnipt as usize);

        let pos = self.positions[node];
        if pos == Vector3::zero() {
            panic!("Tried to unsubdivide a node without position!");
        }

        self.nodes[node] = create_voxel(50, 50, 255);
    }

    pub fn put_in_voxel(&mut self, pos: Vector3<f32>, _: u32, depth: u32) {
        loop {
            let (node, node_depth, _) = self.find_voxel(pos, None);
            if depth == node_depth {
                self.nodes[node] = create_voxel(255, 255, 50);
                return;
            } else {
                self.subdivide(node, 0x00000000, node_depth + 1);
            }
        }
    }

    /// Returns (index, depth, pos)
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

            if self.get_node(node_index + child_index) >= VOXEL_OFFSET
                || depth == max_depth.unwrap_or(u32::MAX)
            {
                return (node_index + child_index, depth, node_pos);
            }

            node_index = self.get_node(node_index + child_index) as usize;
        }
    }

    // // Gonna use hole filling instead for now
    // // Reallocates and rebuilds the entire octree
    // pub fn rebuild(&mut self) {
    //     let mut new_octree = Octree::new(0);

    //     use std::collections::VecDeque;
    //     let mut queue = VecDeque::new();

    //     for child_index in 0..8 {
    //         let child_depth = 1;
    //         let child_pos = Octree::pos_offset(child_index, child_depth);
    //         queue.push_back((child_index, child_depth, child_pos));
    //     }

    //     while let Some((node_index, depth, pos)) = queue.pop_front() {
    //         let tnipt = self.nodes[node_index as usize];
    //         if tnipt >= VOXEL_OFFSET {
    //             let voxel_index = tnipt - VOXEL_OFFSET;
    //             if voxel_index == 0 {
    //                 continue;
    //             }
    //             self.voxel_positions[voxel_index as usize] = pos;
    //         } else {
    //             for child_index in 0..8 {
    //                 let new_index = tnipt as usize + child_index;
    //                 let new_depth = depth + 1;
    //                 let new_pos = pos + Octree::pos_offset(child_index, new_depth);
    //                 stack.push((new_index, new_depth, new_pos));
    //             }
    //         }
    //     }
    // }

    pub fn expanded(&self, size: usize) -> Vec<u32> {
        let mut nodes = self.nodes.clone();
        nodes.extend(std::iter::repeat(0).take(size - self.nodes.len()));

        nodes
    }

    pub fn raw_data(&self) -> &Vec<u32> {
        &self.nodes
    }

    pub fn pos_offset(child_index: usize, depth: u32) -> Vector3<f32> {
        let x = (child_index >> 2) & 1;
        let y = (child_index >> 1) & 1;
        let z = child_index & 1;

        (Vector3::new(x as f32, y as f32, z as f32) * 2.0 - Vector3::new(1.0, 1.0, 1.0))
            / (1 << depth) as f32
    }
}

fn create_node(value: usize) -> u32 {
    (value as u32) << 4
}

// 0, 0, 0 is empty
fn create_voxel(r: u8, g: u8, b: u8) -> u32 {
    let colour = (r as u32) << 16 | (g as u32) << 8 | b as u32;
    (VOXEL_OFFSET + colour) << 4
}

#[allow(dead_code)]
fn count_bits(mut n: u8) -> usize {
    let mut count = 0;
    while n != 0 {
        n = n & (n - 1);
        count += 1;
    }
    return count;
}

impl std::fmt::Debug for Octree {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Nodes ({}):\n", self.nodes.len())?;
        let mut c = 0;
        for value in &self.nodes {
            let pos = self.positions[c];
            if *value >= VOXEL_OFFSET << 4 {
                write!(
                    f,
                    "  Voxel: {} ({}, {}, {})\n",
                    (*value >> 4) - VOXEL_OFFSET,
                    pos.x,
                    pos.y,
                    pos.z
                )?;
            } else {
                write!(
                    f,
                    "  Node: {} ({}, {}, {})\n",
                    *value >> 4,
                    pos.x,
                    pos.y,
                    pos.z
                )?;
            }

            c += 1;
            if c % 8 == 0 {
                write!(f, "\n")?;
            }
        }

        Ok(())
    }
}
