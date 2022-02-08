fn rotate_x(v: vec3<f32>, angle: f32) -> vec3<f32> {
    return vec3<f32>(
        v.x,
        v.y * cos(angle) - v.z * sin(angle),
        v.y * sin(angle) + v.z * cos(angle)
    );
}

fn rotate_y(v: vec3<f32>, angle: f32) -> vec3<f32> {
    return vec3<f32>(
        v.x * cos(angle) + v.z * sin(angle),
        v.y,
        v.z * cos(angle) - v.x * sin(angle)
    );
}

fn rotate_z(v: vec3<f32>, angle: f32) -> vec3<f32> {
    return vec3<f32>(
        v.x * cos(angle) - v.y * sin(angle),
        v.x * sin(angle) + v.y * cos(angle),
        v.z
    );
}

fn rotationMatrix(axis: vec3<f32>, angle: f32) -> mat4x4<f32> {
    let s = sin(angle);
    let c = cos(angle);
    let oc = 1.0 - c;
    
    return mat4x4<f32>(oc * axis.x * axis.x + c,           oc * axis.x * axis.y - axis.z * s,  oc * axis.z * axis.x + axis.y * s,  0.0,
                oc * axis.x * axis.y + axis.z * s,  oc * axis.y * axis.y + c,           oc * axis.y * axis.z - axis.x * s,  0.0,
                oc * axis.z * axis.x - axis.y * s,  oc * axis.y * axis.z + axis.x * s,  oc * axis.z * axis.z + c,           0.0,
                0.0,                                0.0,                                0.0,                                1.0);
}

fn rotate(v: vec3<f32>, axis: vec3<f32>, angle: f32) -> vec3<f32> {
	let m = rotationMatrix(axis, angle);
	return (m * vec4<f32>(v, 1.0)).xyz;
}

fn rand(co: vec2<f32>) -> f32 {
    return fract(sin(dot(co, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}