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

var<workgroup> counter: atomic<u32>;

let BLOCK_OFFSET = 2147483648u;

struct Node {
    value: u32;
    pointing: u32;
};

fn get_node(index: u32) -> u32 {
    return n.data[index];
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

        // Wait for global lock
        // loop {
        //     if (atomicCompareExchangeWeak(&n.lock, 0u, 1u).exchanged) {
        //         break;
        //     }
        //     break;
        // }

        let tnipt = get_node(node_index + child_index);
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

fn put_in_voxel(pos: vec3<f32>, block_id: u32, depth: u32) {
    loop {
        let found_voxel = find_voxel(pos, depth);
        if (found_voxel.depth >= depth) {
            n.data[found_voxel.index] = BLOCK_OFFSET + block_id;

            // Release global lock
            // atomicStore(&n.lock, 0u);
            return;
        } else {
            n.data[found_voxel.index] = add_voxels();

            // Release global lock
            // atomicStore(&n.lock, 0u);
        }
    }
}

fn sdf(pos: vec3<f32>) -> f32 {
    var v = 0.0;

    // Basic shape of island
    v = v + box(pos, vec3<f32>(0.7, 0.1, 0.7)) - 0.1;

    // Some basic noise
    let scale = 1.6;
    let base_noise = simplexNoise3(pos * scale) + 0.5 * simplexNoise3(pos * scale * 2.0);
    v = v + 0.07 * base_noise;

    // Distance from center
    let dist = sqrt(pos.x * pos.x + pos.z * pos.z);

    // Spikes on bottom of island
    let cone = cone(pos * vec3<f32>(1.5, -1.5, 1.5) - vec3<f32>(0.0, 1.0, 0.0), vec2<f32>(0.5, 0.5), 0.9) - 0.1;
    v = smin(v, cone, 0.2);

    let scale = vec3<f32>(2.3, 0.4, 2.3);
    var spike_noise = simplexNoise3(pos * scale) + 0.5 * simplexNoise3(pos * scale * 2.0);
    let height_bias = smoothStep(0.0, -1.5, pos.y) + smoothStep(0.0, 0.2, pos.y);
    spike_noise = spike_noise + 1.6 * dist + height_bias * 2.0 - 1.0;
    // v = smin(v, spike_noise, u.misc1);
    v = v + 0.3 * spike_noise;

    // let edge_distance = 0.7;
    // let edge = min(min(2.5 * -smoothStep(edge_distance, 0.9, abs(pos.x)), 
    //                    2.5 * -smoothStep(edge_distance, 0.9, abs(pos.z))), 
    //                    2.5 * -smoothStep(0.0, 1.0 - 0.8 * dist, abs(pos.y)));

    // v = v + edge;

    // // Spikes on the bottom
    // let spikes = simplexNoise3(pos * vec3<f32>(1.6, 0.8, 1.6));

    // // v = max(v, spikes);
    // v = v + spikes;

    return v;
}

[[stage(compute), workgroup_size(32)]]
fn main([[builtin(global_invocation_id)]] global_id: vec3<u32>) {
    let id = global_id.x * u.dispatch_size + global_id.y; // ) * 24929u) % 16777216u;
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

    let voxel_size = 2.0 / f32(1u << u.depth);

    let uurrgghh = u.misc1;
    let uurrgghh = n.data[id];
    
    // loop {
    //     if (atomicLoad(&n.lock) == id) {
    //         // atomicStore(&n.lock, 1u);
    //         break;
    //     }
    // }

    let v = sdf(pos);
    if (v < 0.0) {
        let above = pos + vec3<f32>(0.0, voxel_size, 0.0);
        let above_sdf = sdf(above);
        if (above_sdf > 0.0) {
            put_in_voxel(pos, 3u, u.depth);
        } else {
            put_in_voxel(pos, 1u, u.depth);
        }
    }

    // if (atomicAdd(&counter, 1u) < 256u) {
    //     put_in_voxel(pos, 3u, u.depth);
    // } else {
    //     put_in_voxel(pos, 1u, u.depth);
    // }

    // atomicStore(&n.lock, (atomicLoad(&n.lock) + 24929u) % 16777216u);
}