// Should be same as main.rs:Unifroms
struct Uniforms {
    camera: mat4x4<f32>;
    camera_inverse: mat4x4<f32>;
    dimensions: vec4<f32>;
    sun_dir: vec4<f32>;
    pause_adaptive: bool;
    show_steps: bool;
    show_hits: bool;
    shadows: bool;
    misc_value: f32;
    misc_bool: bool;
};

struct U32s {
    data: [[stride(4)]] array<u32>;
};

struct AtomicU32s {
    counter: atomic<u32>;
    data: [[stride(4)]] array<u32>;
};

[[group(0), binding(0)]]
var<uniform> u: Uniforms; // uniforms
[[group(0), binding(1)]]
var<storage, read_write> n: U32s; // nodes


let VOXEL_OFFSET = 134217728u;

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

fn node(i: u32) -> u32 {
    return n.data[i] >> 4u;
}

struct Voxel {
    value: u32;
    pos: vec3<f32>;
    depth: u32;
};

// Returns leaf containing position
fn find_voxel(pos: vec3<f32>, primary: bool) -> Voxel {
    var node_index = 0u;
    var node_pos = vec3<f32>(0.0);
    var depth = 0u;
    loop {
        depth = depth + 1u;

        var p: vec3<u32>;
        if (u.misc_bool) {
            p = vec3<u32>(
                u32(pos.x >= node_pos.x),
                u32(pos.y >= node_pos.y),
                u32(pos.z >= node_pos.z)
            );
        } else {
            p = vec3<u32>(
                u32(pos.x > node_pos.x),
                u32(pos.y > node_pos.y),
                u32(pos.z > node_pos.z)
            );
        }
        let child_index = p.x * 4u + p.y * 2u + p.z;

        node_pos = node_pos + (vec3<f32>(p) * 2.0 - 1.0) / f32(1u << depth);

        let p = node_index + child_index;

        // Increment counters
        let value = n.data[p];
        if (primary && (value & 15u) < 15u && !u.pause_adaptive) {
            n.data[p] = value + 1u;
        }

        // tnipt: thing node is pointing to ;)
        // Lets be honest, you dont know how to name variables either
        let tnipt = node(p);
        if (tnipt >= VOXEL_OFFSET) {
            return Voxel(p, node_pos, depth);
        }

        node_index = tnipt;
    }

    // Should never get here
    return Voxel(0u, vec3<f32>(0.0), 0u);
}

fn in_bounds(v: vec3<f32>) -> bool {
    let s = step(vec3<f32>(-1.0), v) - step(vec3<f32>(1.0), v);
    return (s.x * s.y * s.z) > 0.5; 
}

struct HitInfo {
    hit: bool;
    value: u32;
    pos: vec3<f32>;
    normal: vec3<f32>;
    steps: u32;
    depth: u32;
};

fn octree_ray(r: Ray, primary: bool) -> HitInfo {
    var pos = r.pos;
    let dir_mask = vec3<f32>(r.dir == vec3<f32>(0.0));
    var dir = r.dir + dir_mask * 0.000001;

    var dist = 0.0;
    if (!in_bounds(r.pos)) {
        // Get position on surface of the octree
        dist = ray_box_dist(r, vec3<f32>(-1.0), vec3<f32>(1.0));
        if (dist == 0.0){
            return HitInfo(false, 0u, vec3<f32>(0.0), vec3<f32>(0.0), 0u, 0u);
        }

        pos = r.pos + dir * dist;
    }

    let r_sign = sign(dir);

    var voxel = Voxel(0u, vec3<f32>(0.0), 0u);
    var voxel_pos = pos;
    var steps = 0u;
    var normal = trunc(pos * 1.000001);
    loop {
        voxel = find_voxel(voxel_pos, primary);
        if (!u.pause_adaptive || !u.show_hits) {
            let tnipt = node(voxel.value) - VOXEL_OFFSET;
            if (tnipt > 0u) {
                break;
            }
        } else {
            let value = n.data[voxel.value];
            if ((value & 15u) > 0u) {
                break;
            }
        }

        let voxel_size = 2.0 / f32(1u << voxel.depth);
        let t_max = (voxel.pos - pos + r_sign * voxel_size / 2.0) / dir;

        // https://www.shadertoy.com/view/4dX3zl (good old shader toy)
        let mask = vec3<f32>(t_max.xyz <= min(t_max.yzx, t_max.zxy));
        normal = mask * -r_sign;

        let t_current = min(min(t_max.x, t_max.y), t_max.z);
        voxel_pos = pos + dir * t_current - normal * 0.000002;

        if (!in_bounds(voxel_pos)) {
            return HitInfo(false, 0x20202000u, vec3<f32>(0.0), vec3<f32>(0.0), steps, voxel.depth);
        }

        steps = steps + 1u;
        if (steps > 100u) {
            return HitInfo(true, 0xFF000000u, voxel_pos, normal, steps, 100u);
        }
    }

    return HitInfo(true, voxel.value, voxel_pos, normal, steps, voxel.depth);
}

[[stage(fragment)]]
fn fs_main(in: FSIn) -> [[location(0)]] vec4<f32> {
    var output_colour = vec3<f32>(0.0, 0.0, 0.0);
    let clip_space = get_clip_space(in.frag_pos, u.dimensions.xy);

    let pos = u.camera_inverse * vec4<f32>(0.0, 0.0, 0.0, 1.0);
    let dir = u.camera_inverse * vec4<f32>(clip_space.x, clip_space.y, 1.0, 1.0);
    let pos = pos.xyz / pos.w;
    let dir = normalize(dir.xyz / dir.w - pos);
    var ray = Ray(pos.xyz, dir.xyz);

    let hit = octree_ray(ray, true);
    // output_colour = vec3<f32>(hit.pos);
    if (u.show_steps) {
        output_colour = vec3<f32>(f32(hit.steps) / 64.0);
    } else {
        if (hit.hit) {
            if (u.show_hits) {
                output_colour = vec3<f32>(f32(n.data[hit.value] & 15u) / 15.0);
            } else {
                let sun_dir = normalize(u.sun_dir.xyz);

                let ambient = 0.3;
                var diffuse = max(dot(hit.normal, -sun_dir), 0.0);

                if (u.shadows) {
                    let shadow_hit = octree_ray(Ray(hit.pos + hit.normal * 0.0000025, -sun_dir), true);
                    if (shadow_hit.hit) {
                        diffuse = 0.0;
                    }
                }

                let value = node(hit.value) - VOXEL_OFFSET;
                let colour = vec3<f32>(unpack_u8(value).yzw) / 255.0;
                output_colour = (ambient + diffuse) * colour;
            }
        } else {
            output_colour =  vec3<f32>(0.2);
        }
    }

    // let ahha = u.dimensions.x * u.dimensions.y;
    // output_colour = vec3<f32>(f32(atomicAdd(&d.atomic_int, 1u)) / ahha);

    // if (atomicLoad(&d.atomic_int) > u32(ahha)) {
    //     atomicStore(&d.atomic_int, 0u);
    // }

    // let pos = u.misc_value * vec3<f32>(clip_space, u.misc_value);
    // let value = find_voxel(pos, true).value;

    // output_colour = vec3<f32>(unpack_u8(value).yzw);
    // output_colour = pos;

    return vec4<f32>(pow(clamp(output_colour, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(f32(u.misc_bool) * -1.2 + 2.2)), 0.5);
}
