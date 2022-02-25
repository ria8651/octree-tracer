struct Uniforms {
    dispatch_size: u32;
    misc1: f32;
    misc2: f32;
    misc3: f32;
};

struct AtomicU32s {
    counter: atomic<u32>;
    data: [[stride(4)]] array<u32>;
};

[[group(0), binding(0)]]
var<uniform> u: Uniforms; // Uniforms
[[group(0), binding(1)]]
var<storage, read_write> n: AtomicU32s; // Nodes

let BLOCK_OFFSET = 2147483648u;

[[stage(compute), workgroup_size(16)]]
fn main([[builtin(global_invocation_id)]] global_id: vec3<u32>) {
    let id = global_id.x * u.dispatch_size + global_id.y;

    let uurrgghh = u.misc1;
    let uurrgghh = n.data[id];

    let index = atomicAdd(&n.counter, 1u);
    // Value
    n.data[index * 2u] = BLOCK_OFFSET + 8u;
    // Pointer
    n.data[index * 2u + 1u] = BLOCK_OFFSET + 1u;
}