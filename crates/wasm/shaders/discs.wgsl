// Instanced discs drawn straight from the simulation's position buffer (no readback), and
// translucent overlay rectangles (box outlines, highlights) on top.

struct RParams {
    // The visible region of the box (σ).
    view_lo: vec2<f32>,
    view_hi: vec2<f32>,
    // Smallest drawn radius (σ), so discs stay visible when zoomed out.
    min_radius: f32,
    highlight: u32,
    // If nonzero, colour by speed: t = |v_int| · speed_scale from cool_color to hot_color.
    speed_scale: f32,
    _pad: f32,
    radii: vec4<f32>,
    colors: array<vec4<f32>, 4>,
    highlight_color: vec4<f32>,
    cool_color: vec4<f32>,
    hot_color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> R: RParams;
@group(0) @binding(1) var<storage, read> pos: array<vec4<u32>>;
@group(0) @binding(2) var<storage, read> species: array<u32>;
@group(0) @binding(3) var<storage, read> ids: array<u32>;
@group(0) @binding(5) var<storage, read> vel: array<vec2<i32>>;

fn to_clip(p: vec2<f32>) -> vec2<f32> {
    return (p - R.view_lo) / (R.view_hi - R.view_lo) * 2.0 - 1.0;
}

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

fn coord(x: vec2<u32>) -> f32 {
    return f32(x.y) + ldexp(f32(x.x), -32);
}

fn corner(vi: u32) -> vec2<f32> {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
    );
    return corners[vi];
}

// The highlight pipeline draws only the highlighted particle, enlarged, on top of the rest.
override highlight_only: bool = false;

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
    let c = corner(vi);
    let p = pos[ii];
    let s = species[ii];
    let center = to_clip(vec2<f32>(coord(p.xy), coord(p.zw)));
    var r = max(R.radii[s], R.min_radius);
    var color = R.colors[s];
    if (R.speed_scale > 0.0) {
        let v = vec2<f32>(vel[ii]);
        color = mix(R.cool_color, R.hot_color, clamp(length(v) * R.speed_scale, 0.0, 1.0));
    }
    var out: VOut;
    if (highlight_only) {
        if (ids[ii] != R.highlight) {
            out.clip = vec4<f32>(2.0, 2.0, 2.0, 1.0);
            return out;
        }
        r = max(r, 3.0 * R.min_radius) * 2.0;
        color = R.highlight_color;
    }
    out.clip = vec4<f32>(center + c * r / (R.view_hi - R.view_lo) * 2.0, 0.0, 1.0);
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

// Overlay rectangles: (x0, y0, x1, y1) in σ and an RGBA colour, alpha-blended.
struct Rect {
    lo: vec2<f32>,
    hi: vec2<f32>,
    color: vec4<f32>,
}

@group(0) @binding(4) var<storage, read> rects: array<Rect>;

struct ROut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_rect(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> ROut {
    let r = rects[ii];
    let t = corner(vi) * 0.5 + 0.5;
    let p = to_clip(mix(r.lo, r.hi, t));
    var out: ROut;
    out.clip = vec4<f32>(p, 0.0, 1.0);
    out.color = r.color;
    return out;
}

@fragment
fn fs_rect(in: ROut) -> @location(0) vec4<f32> {
    return in.color;
}
