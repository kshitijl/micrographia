// A lattice Boltzmann fluid drawn straight from its density and temperature buffers: liquid to
// vapour by density, optionally tinted by temperature.

struct F {
    w: u32,
    h: u32,
    // 0: by density; 1: by temperature (shaded by density).
    mode: u32,
    _p: u32,
    view: vec4<f32>,
    // Densities mapped to the vapour and liquid colours.
    rho_v: f32,
    rho_l: f32,
    // Temperatures mapped to the cool and hot colours.
    t_lo: f32,
    t_hi: f32,
    liquid: vec4<f32>,
    vapour: vec4<f32>,
    cool: vec4<f32>,
    hot: vec4<f32>,
    wall: vec4<f32>,
}

@group(0) @binding(0) var<uniform> R: F;
@group(0) @binding(1) var<storage, read> rho: array<f32>;
@group(0) @binding(2) var<storage, read> temp: array<f32>;

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VOut {
    let p = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    var out: VOut;
    out.clip = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    out.uv = p;
    return out;
}

@fragment
fn fs(in: VOut) -> @location(0) vec4<f32> {
    let s = mix(R.view.xy, R.view.zw, in.uv);
    if (s.x < 0.0 || s.y < 0.0 || s.x >= f32(R.w) || s.y >= f32(R.h)) {
        return R.vapour * 0.6;
    }
    let x = u32(s.x);
    let y = u32(s.y);
    if (y == 0u || y == R.h - 1u) {
        return R.wall;
    }
    let i = y * R.w + x;
    let f = clamp((rho[i] - R.rho_v) / (R.rho_l - R.rho_v), 0.0, 1.0);
    var c = mix(R.vapour, R.liquid, f);
    if (R.mode == 1u) {
        let t = clamp((temp[i] - R.t_lo) / (R.t_hi - R.t_lo), 0.0, 1.0);
        c = mix(R.cool, R.hot, t) * (0.45 + 0.55 * f);
    }
    return vec4<f32>(c.rgb, 1.0);
}
