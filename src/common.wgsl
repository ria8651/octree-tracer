fn rotate_x(v: vec3<f32>, angle: f32) -> vec3<f32> {
    return vec3<f32>(
        v.x,
        v.y * cos(angle) - v.z * sin(angle),
        v.y * sin(angle) + v.z * cos(angle),
    );
}

fn rotate_y(v: vec3<f32>, angle: f32) -> vec3<f32> {
    return vec3<f32>(
        v.x * cos(angle) + v.z * sin(angle),
        v.y,
        v.z * cos(angle) - v.x * sin(angle),
    );
}

fn rotate_z(v: vec3<f32>, angle: f32) -> vec3<f32> {
    return vec3<f32>(
        v.x * cos(angle) - v.y * sin(angle),
        v.x * sin(angle) + v.y * cos(angle),
        v.z,
    );
}

fn rotationMatrix(axis: vec3<f32>, angle: f32) -> mat4x4<f32> {
    let s = sin(angle);
    let c = cos(angle);
    let oc = 1.0 - c;
    return mat4x4<f32>(oc * axis.x * axis.x + c, oc * axis.x * axis.y - axis.z * s, oc * axis.z * axis.x + axis.y * s, 0.0, oc * axis.x * axis.y + axis.z * s, oc * axis.y * axis.y + c, oc * axis.y * axis.z - axis.x * s, 0.0, oc * axis.z * axis.x - axis.y * s, oc * axis.y * axis.z + axis.x * s, oc * axis.z * axis.z + c, 0.0, 0.0, 0.0, 0.0, 1.0);
}

fn rotate(v: vec3<f32>, axis: vec3<f32>, angle: f32) -> vec3<f32> {
    let m = rotationMatrix(axis, angle);
    return (m * vec4<f32>(v, 1.0)).xyz;
}

fn rand(co: vec2<f32>) -> f32 {
    return fract(sin(dot(co, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

//  MIT License. © Ian McEwan, Stefan Gustavson, Munrocket
//
fn permute4(x: vec4<f32>) -> vec4<f32> {
    return ((x * 34. + 1.) * x) % vec4<f32>(289.); }
fn taylorInvSqrt4(r: vec4<f32>) -> vec4<f32> {
    return 1.79284291400159 - 0.85373472095314 * r; }

fn simplexNoise3(v: vec3<f32>) -> f32 {
    let C = vec2<f32>(1. / 6., 1. / 3.);
    let D = vec4<f32>(0., 0.5, 1., 2.);

    // First corner
    var i: vec3<f32> = floor(v + dot(v, C.yyy));
    let x0 = v - i + dot(i, C.xxx);

    // Other corners
    let g = step(x0.yzx, x0.xyz);
    let l = 1.0 - g;
    let i1 = min(g.xyz, l.zxy);
    let i2 = max(g.xyz, l.zxy);

    // x0 = x0 - 0. + 0. * C
    let x1 = x0 - i1 + 1. * C.xxx;
    let x2 = x0 - i2 + 2. * C.xxx;
    let x3 = x0 - 1. + 3. * C.xxx;

    // Permutations
    i = i % vec3<f32>(289.);
    let p = permute4(permute4(permute4(
        i.z + vec4<f32>(0., i1.z, i2.z, 1.),
    ) + i.y + vec4<f32>(0., i1.y, i2.y, 1.)) + i.x + vec4<f32>(0., i1.x, i2.x, 1.));

    // Gradients (NxN points uniformly over a square, mapped onto an octahedron.)
    var n_: f32 = 1. / 7.; // N=7
    let ns = n_ * D.wyz - D.xzx;
    let j = p - 49. * floor(p * ns.z * ns.z); // mod(p, N*N)
    let x_ = floor(j * ns.z);
    let y_ = floor(j - 7.0 * x_); // mod(j, N)
    let x = x_ * ns.x + ns.yyyy;
    let y = y_ * ns.x + ns.yyyy;
    let h = 1.0 - abs(x) - abs(y);
    let b0 = vec4<f32>(x.xy, y.xy);
    let b1 = vec4<f32>(x.zw, y.zw);
    let s0 = floor(b0) * 2.0 + 1.0;
    let s1 = floor(b1) * 2.0 + 1.0;
    let sh = -step(h, vec4<f32>(0.));
    let a0 = b0.xzyw + s0.xzyw * sh.xxyy ;
    let a1 = b1.xzyw + s1.xzyw * sh.zzww ;
    var p0: vec3<f32> = vec3<f32>(a0.xy, h.x);
    var p1: vec3<f32> = vec3<f32>(a0.zw, h.y);
    var p2: vec3<f32> = vec3<f32>(a1.xy, h.z);
    var p3: vec3<f32> = vec3<f32>(a1.zw, h.w);

    // Normalise gradients
    let norm = taylorInvSqrt4(vec4<f32>(dot(p0, p0), dot(p1, p1), dot(p2, p2), dot(p3, p3)));
    p0 = p0 * norm.x;
    p1 = p1 * norm.y;
    p2 = p2 * norm.z;
    p3 = p3 * norm.w;

    // Mix final noise value
    var m: vec4<f32> = 0.6 - vec4<f32>(dot(x0, x0), dot(x1, x1), dot(x2, x2), dot(x3, x3));
    m = max(m, vec4<f32>(0.));
    m = m * m;
    return 42. * dot(m * m, vec4<f32>(dot(p0, x0), dot(p1, x1), dot(p2, x2), dot(p3, x3)));
}

// unsigned rounded box: s=size
fn box(p: vec3<f32>, s: vec3<f32>) -> f32 {
    let q = abs(p) - s;
    return length(max(q, vec3<f32>(0.0))) + min(max(max(q.x, q.y), q.z), 0.0);
}

fn cone(p: vec3<f32>, c: vec2<f32>, h: f32) -> f32 {
    var p1: vec3<f32>;
    var c1: vec2<f32>;
    var h1: f32;
    var q: vec2<f32>;
    var w: vec2<f32>;
    var a: vec2<f32>;
    var b: vec2<f32>;
    var k: f32;
    var d: f32;
    var s: f32;

    p1 = p;
    c1 = c;
    h1 = h;
    let e7: f32 = h1;
    let e8: vec2<f32> = c1;
    let e10: vec2<f32> = c1;
    q = (e7 * vec2<f32>((e8.x / e10.y), -(1.0)));
    let e18: vec3<f32> = p1;
    let e20: vec3<f32> = p1;
    let e23: vec3<f32> = p1;
    w = vec2<f32>(length(e20.xz), e23.y);
    let e27: vec2<f32> = w;
    let e28: vec2<f32> = q;
    let e31: vec2<f32> = w;
    let e32: vec2<f32> = q;
    let e36: vec2<f32> = q;
    let e37: vec2<f32> = q;
    let e44: vec2<f32> = w;
    let e45: vec2<f32> = q;
    let e49: vec2<f32> = q;
    let e50: vec2<f32> = q;
    a = (e27 - (e28 * clamp((dot(e44, e45) / dot(e49, e50)), 0.0, 1.0)));
    let e59: vec2<f32> = w;
    let e60: vec2<f32> = q;
    let e61: vec2<f32> = w;
    let e63: vec2<f32> = q;
    let e68: vec2<f32> = w;
    let e70: vec2<f32> = q;
    b = (e59 - (e60 * vec2<f32>(clamp((e68.x / e70.x), 0.0, 1.0), 1.0)));
    let e81: vec2<f32> = q;
    let e83: vec2<f32> = q;
    k = sign(e83.y);
    let e89: vec2<f32> = a;
    let e90: vec2<f32> = a;
    let e94: vec2<f32> = b;
    let e95: vec2<f32> = b;
    let e99: vec2<f32> = a;
    let e100: vec2<f32> = a;
    let e104: vec2<f32> = b;
    let e105: vec2<f32> = b;
    d = min(dot(e99, e100), dot(e104, e105));
    let e109: f32 = k;
    let e110: vec2<f32> = w;
    let e112: vec2<f32> = q;
    let e115: vec2<f32> = w;
    let e117: vec2<f32> = q;
    let e122: f32 = k;
    let e123: vec2<f32> = w;
    let e125: vec2<f32> = q;
    let e129: f32 = k;
    let e130: vec2<f32> = w;
    let e132: vec2<f32> = q;
    let e135: vec2<f32> = w;
    let e137: vec2<f32> = q;
    let e142: f32 = k;
    let e143: vec2<f32> = w;
    let e145: vec2<f32> = q;
    s = max((e129 * ((e130.x * e132.y) - (e135.y * e137.x))), (e142 * (e143.y - e145.y)));
    let e152: f32 = d;
    let e155: f32 = s;
    return (sqrt(e152) * sign(e155));
}

fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (a - b) / k, 0.0, 1.0);
    return mix(a, b, h) - k * h * (1.0 - h);
}