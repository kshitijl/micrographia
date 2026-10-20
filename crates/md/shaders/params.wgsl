// Shared by every MD kernel module: the parameter block (mirrors engine::Params, always at
// binding 0) and the exact fixed-point pair geometry. Requires fixed64.wgsl.

struct Params {
    n: u32,
    ncx: u32,
    ncy: u32,
    ncell: u32,
    box_x: vec2<u32>,
    box_y: vec2<u32>,
    half_x: vec2<u32>,
    half_y: vec2<u32>,
    box_f: vec2<f32>,
    cell_scale: vec2<f32>,
    rc2: f32,
    eps4: f32,
    sigma2: f32,
    shift: f32,
    kick_c: vec4<f32>,
    flip_index: u32,
    flip_word: u32,
    flip_mask: u32,
    _pad: u32,
    // Neighbour lists: squared list radius (rc + skin)², squared half skin, capacity per
    // particle, and bits per axis of the Morton cell index.
    list_r2: f32,
    half_skin2: f32,
    cap: u32,
    mbits: u32,
}

@group(0) @binding(0) var<uniform> P: Params;

// A coordinate in σ, for cell assignment only (any rounding here is harmless: cells are wider
// than the cutoff by a margin).
fn coord_f32(x: vec2<u32>) -> f32 {
    return f32(x.y) + ldexp(f32(x.x), -32);
}

fn spread_bits(v: u32) -> u32 {
    var x = v & 0x0000ffffu;
    x = (x | (x << 8u)) & 0x00ff00ffu;
    x = (x | (x << 4u)) & 0x0f0f0f0fu;
    x = (x | (x << 2u)) & 0x33333333u;
    x = (x | (x << 1u)) & 0x55555555u;
    return x;
}

// Morton (Z-order) index of a cell.
fn morton(cx: u32, cy: u32) -> u32 {
    return spread_bits(cx) | (spread_bits(cy) << 1u);
}

fn cell_coords(p: vec4<u32>) -> vec2<u32> {
    let cx = min(u32(coord_f32(p.xy) * P.cell_scale.x), P.ncx - 1u);
    let cy = min(u32(coord_f32(p.zw) * P.cell_scale.y), P.ncy - 1u);
    return vec2<u32>(cx, cy);
}

// Exact minimum image of a separation d in [-L, L).
fn min_image64(d: vec2<u32>, l: vec2<u32>, half: vec2<u32>) -> vec2<u32> {
    if (!lt64(d, half)) {
        return sub64(d, l);
    }
    if (lt64(d, neg64(half))) {
        return add64(d, l);
    }
    return d;
}

// Separation in σ through units of 2^-28 σ; antisymmetric bit for bit. Beyond 8 σ returns a
// huge value so the pair fails the cutoff test.
fn sep_f32(d: vec2<u32>) -> f32 {
    let hi = bitcast<i32>(d.y);
    if (hi < -8 || hi > 7) {
        return 1e30;
    }
    let neg = hi < 0;
    let a = select(d, neg64(d), neg);
    let m = ldexp(f32((a.y << 28u) | (a.x >> 4u)), -28);
    return select(m, -m, neg);
}

fn wrap64(x: vec2<u32>, l: vec2<u32>) -> vec2<u32> {
    if (is_neg64(x)) {
        return add64(x, l);
    }
    if (!lt64(x, l)) {
        return sub64(x, l);
    }
    return x;
}
