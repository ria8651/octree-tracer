struct Uniforms {
    camera: mat4x4<f32>;
    camera_inverse: mat4x4<f32>;
    dimensions: vec4<f32>;
    misc_value: f32;
    misc_bool: bool;
};

[[group(0), binding(0)]]
var<uniform> u: Uniforms;

struct Data {
    data: [[stride(4)]] array<u32>;
};

[[group(0), binding(1)]]
var<storage, read_write> d: Data;

[[stage(vertex)]]
fn vs_main([[builtin(vertex_index)]] in_vertex_index: u32) -> [[builtin(position)]] vec4<f32> {
    var x = 0.0;
    var y = 0.0;

    if (in_vertex_index == 0u) {
        x = -1.0;
        y = -1.0;
    } else if (in_vertex_index == 1u) {
        x = 1.0;
        y = -1.0;
    } else if (in_vertex_index == 2u) {
        x = -1.0;
        y = 1.0;
    } else if (in_vertex_index == 3u) {
        x = 1.0;
        y = 1.0;
    }

    return vec4<f32>(x, y, 0.0, 1.0);
}

fn get_clip_space(frag_pos: vec4<f32>, dimensions: vec2<f32>) -> vec2<f32> {
    var clip_space = frag_pos.xy / dimensions * 2.0;
    clip_space = clip_space - 1.0;
    clip_space = clip_space * vec2<f32>(1.0, -1.0);
    return clip_space;
}

struct Ray {
    pos: vec3<f32>;
    dir: vec3<f32>;
};

fn ray_box_dist(r: Ray, vmin: vec3<f32>, vmax: vec3<f32>) -> f32 {
    let v1 = (vmin.x - r.pos.x) / r.dir.x;
    let v2 = (vmax.x - r.pos.x) / r.dir.x;
    let v3 = (vmin.y - r.pos.y) / r.dir.y;
    let v4 = (vmax.y - r.pos.y) / r.dir.y;
    let v5 = (vmin.z - r.pos.z) / r.dir.z;
    let v6 = (vmax.z - r.pos.z) / r.dir.z;
    let v7 = max(max(min(v1, v2), min(v3, v4)), min(v5, v6));
    let v8 = min(min(max(v1, v2), max(v3, v4)), max(v5, v6));
    if (v8 < 0.0 || v7 > v8) {
        return 0.0;
    }
    
    return v7;
}

struct FSIn {
    [[builtin(position)]] frag_pos: vec4<f32>;
};


fn count_bits(n: u32) -> u32 {
    var count = 0u;
    var n = n;
    loop {
        if (n == 0u) { break; }
        n = n & (n - 1u);
        count = count + 1u;
    }
    return count;
}

fn unpack_u8(p: u32) -> vec4<u32> {
    return vec4<u32>(
        (p >> 24u) & 0xFFu,
        (p >> 16u) & 0xFFu,
        (p >> 8u) & 0xFFu,
        p & 0xFFu
    );
}

fn unpack_u24u8(p: u32) -> vec2<u32> {
    return vec2<u32>(
        (p >> 8u) & 0xFFFFFFu,
        p & 0xFFu
    );
}

struct Node {
    mask: u32;
    index: u32;
};

fn get_node(i: u32) -> Node {
    let s = unpack_u24u8(d.data[i]);
    return Node(s.y, s.x);
}

struct Voxel {
    exists: bool;
    value: u32;
};

// Returns leaf containing position
fn find_voxel(pos: vec3<f32>) -> Voxel {
    var node_index = 0u;
    var node_pos = vec3<f32>(0.0);
    var depth = 1u;
    loop {
        let node = get_node(node_index);

        let p = vec3<u32>(
            u32(pos.x > node_pos.x),
            u32(pos.y > node_pos.y),
            u32(pos.z > node_pos.z)
        );
        let child_index = p.x * 4u + p.y * 2u + p.z;

        node_pos = node_pos + (vec3<f32>(p) * 2.0 - 1.0) / f32(1u << depth);

        // Leaf
        if (node.mask == 0u) {
            return Voxel(true, d.data[node_index]);
        }

        // Node
        let bit = (node.mask >> child_index) & 1u;
        if (bool(bit)) {
            let mask = (1u << child_index) - 1u;
            let offset = count_bits(node.mask & mask);
            node_index = node.index + offset;
            // return Voxel(true, 65535u);
        } else {
            break;
        }

        depth = depth + 1u;
    }

    return Voxel(false, 0u);
}

// fn in_bounds(v: vec3<f32>) -> bool {
//     let s = step(vec3<f32>(-1.0), v) - step(vec3<f32>(1.0), v);
//     return (s.x * s.y * s.z) > 0.5; 
// }

[[stage(fragment)]]
fn fs_main(in: FSIn) -> [[location(0)]] vec4<f32> {
    var output_colour = vec3<f32>(0.0, 0.0, 0.0);
    let clip_space = get_clip_space(in.frag_pos, u.dimensions.xy);

    let pos = u.camera_inverse * vec4<f32>(clip_space.x, clip_space.y, 0.0, 1.0);
    let dir = u.camera_inverse * vec4<f32>(clip_space.x, clip_space.y, 1.0, 1.0);
    let pos = pos.xyz / pos.w;
    let dir = normalize(dir.xyz / dir.w - pos);
    var ray = Ray(pos.xyz, dir.xyz);

    let dist = ray_box_dist(ray, vec3<f32>(-1.0), vec3<f32>(1.0));
    if (dist != 0.0) {
        let pos = ray.pos + ray.dir * (dist + 0.001);

        let voxel = find_voxel(pos);

        if (voxel.exists) {
            output_colour = vec3<f32>(unpack_u8(voxel.value).rgb) / 255.0;
        } else {
            output_colour = vec3<f32>(0.3, 0.3, 0.8);
        }
    } else {
        output_colour = vec3<f32>(0.2, 0.2, 0.2);
    }
    
    return vec4<f32>(pow(clamp(output_colour, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(f32(u.misc_bool) * -1.2 + 2.2 )), 0.5);
}


// let grid_size = 4.0;
// let grid = floor((clip_space * 0.5 + 0.5) * grid_size) / grid_size;
// let index = u32((grid.y * grid_size + grid.x) * grid_size);

// output_colour = vec3<f32>(unpack_u8(d.data[index]).rgb) / 255.0;