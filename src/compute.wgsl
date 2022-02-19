struct U32s {
    data: [[stride(4)]] array<u32>;
};
struct AtomicU32s {
    counter: atomic<u32>;
    data: [[stride(4)]] array<u32>;
};
struct CUniforms {
    max_depth: u32;
};


[[group(0), binding(0)]]
var<uniform> u: CUniforms; // Uniforms
[[group(0), binding(1)]]
var<storage, read_write> n: U32s; // Nodes
[[group(0), binding(2)]]
var<storage, read_write> s: AtomicU32s; // Subdivision output
[[group(0), binding(3)]]
var<storage, read_write> us: AtomicU32s;  // Unsubdivision output

let DISPATCH_SIZE_Y = 256u;
let VOXEL_OFFSET = 134217728u;

[[stage(compute), workgroup_size(16)]]
fn main([[builtin(global_invocation_id)]] global_id: vec3<u32>) {
    let id = global_id.x * DISPATCH_SIZE_Y + global_id.y;

    let uurrgghh = u.max_depth;
    let uurrgghh = s.data[id];
    let uurrgghh = us.data[id];

    let node = n.data[id];
    if (node == 0u) {
        return;
    }

    let counter = node & 15u;
    if (counter == 0u && (node >> 4u) < VOXEL_OFFSET) {
        let index = atomicAdd(&us.counter, 1u);
        us.data[index] = id;
    } else if (counter >= 4u && (node >> 4u) > VOXEL_OFFSET) {
        let index = atomicAdd(&s.counter, 1u);
        s.data[index] = id;
    }
}