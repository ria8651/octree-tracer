struct U32s {
    data: [[stride(4)]] array<u32>;
};

[[group(0), binding(0)]]
var<storage, read_write> v: U32s;

[[stage(compute), workgroup_size(1)]]
fn main([[builtin(global_invocation_id)]] global_id: vec3<u32>) {
    v.data[global_id.x] = global_id.x * 5u;
}