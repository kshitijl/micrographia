// A lattice of sites (0 or 1, from the simulation's buffer, no readback), drawn as an image:
// each pixel shows the fraction of occupied sites under it, so a big lattice shows density
// (a thin vapour as a faint haze) rather than aliasing.

struct G {
    w: u32,
    h: u32,
    // Sites averaged per pixel along each axis (≥ 1).
    k: u32,
    hot_rows: u32,
    // The visible region, in sites: (x0, y0, x1, y1); y up.
    view: vec4<f32>,
    filled: vec4<f32>,
    empty: vec4<f32>,
    hot: vec4<f32>,
}

@group(0) @binding(0) var<uniform> R: G;
@group(0) @binding(1) var<storage, read> occ: array<u32>;

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VOut {
    // One triangle covering the screen.
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
        return R.empty * 0.6;
    }
    let x0 = u32(s.x);
    let y0 = u32(s.y);
    var sum = 0u;
    for (var dy = 0u; dy < R.k; dy++) {
        let y = min(y0 + dy, R.h - 1u);
        for (var dx = 0u; dx < R.k; dx++) {
            sum += occ[y * R.w + (x0 + dx) % R.w];
        }
    }
    let f = f32(sum) / f32(R.k * R.k);
    var c = mix(R.empty, R.filled, f);
    if (y0 < R.hot_rows) {
        c = mix(c, R.hot, 0.35);
    }
    return vec4<f32>(c.rgb, 1.0);
}
