// Instanced discs drawn straight from the simulation's position buffer (no readback).

struct RParams {
    box_f: vec2<f32>,
    radius: f32,
    highlight: u32,
    colors: array<vec4<f32>, 4>,
    highlight_color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> R: RParams;
@group(0) @binding(1) var<storage, read> pos: array<vec4<u32>>;
@group(0) @binding(2) var<storage, read> species: array<u32>;

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

fn coord(x: vec2<u32>) -> f32 {
    return f32(x.y) + ldexp(f32(x.x), -32);
}

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
    );
    let c = corners[vi];
    let p = pos[ii];
    let center = vec2<f32>(coord(p.xy), coord(p.zw)) / R.box_f * 2.0 - 1.0;
    var r = R.radius;
    var color = R.colors[species[ii]];
    if (ii == R.highlight) {
        r *= 4.0;
        color = R.highlight_color;
    }
    var out: VOut;
    out.clip = vec4<f32>(center + c * r / R.box_f * 2.0, 0.0, 1.0);
    out.uv = c;
    out.color = color;
    return out;
}

@fragment
fn fs(in: VOut) -> @location(0) vec4<f32> {
    let r2 = dot(in.uv, in.uv);
    if (r2 > 1.0) {
        discard;
    }
    return vec4<f32>(in.color.rgb * (1.0 - 0.18 * r2), in.color.a);
}
