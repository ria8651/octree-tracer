struct Uniforms {
    dispatch_size: u32;
    depth: u32;
    misc1: f32;
    misc2: f32;
    misc3: f32;
};

struct AtomicU32s {
    len: atomic<u32>;
    lock: atomic<u32>; // Becuase entire buffer is interpreted as u64
    data: [[stride(4)]] array<u32>;
};

[[group(0), binding(0)]]
var<uniform> u: Uniforms; // Uniforms
[[group(0), binding(1)]]
var<storage, read_write> n: AtomicU32s; // Nodes

let BLOCK_OFFSET = 2147483648u;

struct Node {
    value: u32;
    pointing: u32;
};

fn get_node(index: u32) -> Node {
    return Node(
        n.data[index * 2u],
        n.data[index * 2u + 1u]
    );
}

fn add_voxels() -> u32 {
    let index = atomicAdd(&n.len, 8u);
    // let index = n.len;
    // n.len = n.len + 8u;
    return index;
}

struct FoundVoxel {
    index: u32;
    depth: u32;
    pos: vec3<f32>;
};

/// Returns (index, depth, pos)
fn find_voxel(
    pos: vec3<f32>,
    max_depth: u32,
) -> FoundVoxel {
    var node_index = 0u;
    var node_pos = vec3<f32>(0.0);
    var depth = 0u;
    loop {
        depth = depth + 1u;

        let p = vec3<u32>(
            u32(pos.x >= node_pos.x),
            u32(pos.y >= node_pos.y),
            u32(pos.z >= node_pos.z),
        );
        let child_index = p.x * 4u + p.y * 2u + p.z;

        node_pos = node_pos + (vec3<f32>(p) * 2.0 - 1.0) / f32(1u << depth);

        let tnipt = get_node(node_index + child_index).pointing;
        if (
            tnipt >= BLOCK_OFFSET
            || tnipt == 0u
            || (depth != 0u && depth == max_depth)
        ) {
            return FoundVoxel(node_index + child_index, depth, node_pos);
        }

        node_index = tnipt;
    }

    return FoundVoxel(0u, 0u, vec3<f32>(0.0));
}

fn put_in_voxel(pos: vec3<f32>, voxel: u32, depth: u32) {
    loop {
        let found_voxel = find_voxel(pos, depth);
        if (found_voxel.depth >= depth) {
            n.data[found_voxel.index * 2u] = voxel;
            n.data[found_voxel.index * 2u + 1u] = BLOCK_OFFSET + 1u;
            return;
        } else {
            n.data[found_voxel.index * 2u] = 16711680u;

            // Wait for global lock
            // loop {
            //     if (atomicCompareExchangeWeak(&n.lock, 0u, 1u)) {
            //         break;
            //     }
            //     break;
            // }

            n.data[found_voxel.index * 2u + 1u] = add_voxels();

            // Release global lock
            // atomicStore(&n.lock, 0u);
        }
    }
}

[[stage(compute), workgroup_size(32)]]
fn main([[builtin(global_invocation_id)]] global_id: vec3<u32>) {
    let id = global_id.x * u.dispatch_size + global_id.y;
    let side_length = 1u << u.depth;
    if (id >= side_length * side_length * side_length) {
        return;
    }

    let pos = vec3<f32>(
        f32(id % side_length),
        f32(id / side_length % side_length),
        f32(id / side_length / side_length),
    ) / f32(side_length);
    let pos = pos * 2.0 - 1.0;

    let uurrgghh = u.misc1;
    let uurrgghh = n.data[id];

    let scale = 1.6;
    var v = simplexNoise3(pos * scale) + 0.5 * simplexNoise3(pos * scale * 2.0) + 0.25 * simplexNoise3(pos * scale * 4.0);

    // v = v - pos.y * 5.0;

    let edge_distance = 8.0;
    let falloff = 10.0;
    let edge = min(min(-falloff * abs(pos.x) + edge_distance, -falloff * abs(pos.z) + edge_distance), -falloff * abs(pos.y) + 2.0);
    v = v + edge;
    
    if (v > 0.0) {
        put_in_voxel(pos, 255u, u.depth);
    }
}