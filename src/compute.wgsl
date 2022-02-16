struct U32s {
    data: [[stride(4)]] array<u32>;
};
[[group(0), binding(0)]]
var<storage, read_write> v: U32s;

struct AtomicU32s {
    counter: atomic<u32>;
    data: [[stride(4)]] array<u32>;
};
[[group(1), binding(0)]]
var<storage, read_write> f: AtomicU32s;

[[stage(compute), workgroup_size(128)]]
fn main([[builtin(global_invocation_id)]] global_id: vec3<u32>) {
    if (v.data[global_id.x] > 8u) {
        let index = atomicAdd(&f.counter, 1u);
        f.data[index] = global_id.x;
    }

    v.data[global_id.x] = 0u;
}