use cgmath::*;

// First palette colour is empty voxel
// const PALETTE: [u32; 3] = [0x00000000, 0x0000FF00, 0x000000FF];
pub const VOXEL_OFFSET: u32 = 134217728;

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Voxel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Voxel {
    pub fn new(r: u8, g: u8, b: u8) -> Voxel {
        Voxel { r, g, b }
    }

    #[allow(dead_code)]
    pub fn from_value(value: u32) -> Voxel {
        let r = (value >> 16) as u8;
        let g = (value >> 8) as u8;
        let b = value as u8;

        Voxel::new(r, g, b)
    }

    pub fn to_value(&self) -> u32 {
        (VOXEL_OFFSET + self.to_cpu_value()) << 4
    }

    pub fn to_cpu_value(&self) -> u32 {
        (self.r as u32) << 16 | (self.g as u32) << 8 | self.b as u32
    }
}

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
    pub fn new(mask: [Voxel; 8]) -> Self {
        let mut nodes = Vec::new();
        let mut positions = Vec::new();
        let hole_stack = Vec::new();

        for i in 0..8 {
            nodes.push(mask[i].to_value());
            positions.push(Octree::pos_offset(i, 1));
        }

        Self {
            nodes,
            positions,
            hole_stack,
        }
    }

    pub fn get_node(&self, index: usize) -> u32 {
        self.nodes[index] >> 4
    }

    pub fn subdivide(&mut self, node: usize, mask: [Voxel; 8], depth: u32) {
        if self.get_node(node) < VOXEL_OFFSET {
            panic!("Node already subdivided!");
        }

        let pos = self.positions[node];
        if let Some(index) = self.hole_stack.pop() {
            self.nodes[node] = create_node(index);

            for i in 0..8 {
                self.nodes[index + i] = mask[i].to_value();
                self.positions[index + i] = pos + Octree::pos_offset(i, depth);
            }
        } else {
            self.nodes[node] = create_node(self.nodes.len());

            for i in 0..8 {
                self.nodes.push(mask[i].to_value());
                self.positions.push(pos + Octree::pos_offset(i, depth));
            }
        }
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

        self.nodes[node] = Voxel::new(255, 0, 0).to_value();
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

pub fn create_node(value: usize) -> u32 {
    (value as u32) << 4
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

impl std::fmt::Debug for Voxel {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({}, {}, {})", self.r, self.g, self.b)
    }
}
