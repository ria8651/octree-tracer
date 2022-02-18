struct U32s {
    data: [[stride(4)]] array<u32>;
};
struct AtomicU32s {
    counter: atomic<u32>;
    data: [[stride(4)]] array<u32>;
};

[[group(0), binding(0)]]
var<storage, read_write> n: U32s;
[[group(0), binding(1)]]
var<storage, read_write> u: AtomicU32s;

let DISPATCH_SIZE_Y = 256u;
let VOXEL_OFFSET = 2147483647u;

[[stage(compute), workgroup_size(16)]]
fn main([[builtin(global_invocation_id)]] global_id: vec3<u32>) {
    let id = global_id.x * DISPATCH_SIZE_Y + global_id.y;

    let uurrgghh = n.data[id];
    let uurrgghh = u.data[id];

    // let counter = v.data[id] & u32(15);
    // if (counter > 4u) {
    //     let index = atomicAdd(&f.counter, 1u);
    //     f.data[index] = VOXEL_OFFSET + id;
    // }
    
    // else if (counter == 0u) {
    //     // n.data[id] = v.data[id] + 1u;
    //     let parent = v.data[id] >> 4u;
    //     if (parent == 0u) {
    //         return;
    //     }

    //     let tnipt = n.data[parent];
    //     if (tnipt < VOXEL_OFFSET) {
    //         for (var i = 0u; i < 8u; i = i + 1u) {
    //             let child_value = n.data[tnipt + i];
    //             if (child_value < VOXEL_OFFSET) {
    //                 return;
    //             } else if (child_value > VOXEL_OFFSET) {
    //                 let voxel_index = child_value - VOXEL_OFFSET;
    //                 let sibling_counter = v.data[id] & u32(15);
    //                 if (sibling_counter != 0u) {
    //                     return;
    //                 }
    //             }
    //         }

    //         let index = atomicAdd(&f.counter, 1u);
    //         f.data[index] = parent;
    //     } else {
    //         // Bad
    //     }
    // }

    // if ((n.data[id] & 15u) < 15u) {
    //     n.data[id] = n.data[id] + 1u;
    // } 
    n.data[id] = n.data[id] & 4294967280u;
}