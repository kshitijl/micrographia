// 3D: lit spheres drawn straight from the simulation's position buffer (no readback), as
// camera-facing quads with an exact ray–sphere test per pixel (so they intersect and occlude
// correctly), and the box's edges as lines.

struct R3 {
    // World (σ) → view space (camera at the origin, looking down −z), and view → clip.
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    radii: vec4<f32>,
    colors: array<vec4<f32>, 4>,
    highlight_color: vec4<f32>,
    cool_color: vec4<f32>,
    mid_color: vec4<f32>,
    hot_color: vec4<f32>,
    background: vec4<f32>,
    edge_color: vec4<f32>,
    // Box size (σ) and the smallest drawn radius (σ).
    box_size: vec4<f32>,
    // Spheres are drawn only if their centre's view-space z is in [slab.x, slab.y] (a cut-away);
    // slab.z, slab.w: the view-space z range of the box, for depth cueing.
    slab: vec4<f32>,
    // 0: colour by species; 1: by speed (|v_int| · speed_scale); 2: by the page's values (by id).
    color_mode: u32,
    speed_scale: f32,
    highlight: u32,
    _p: u32,
}

@group(0) @binding(0) var<uniform> R: R3;
@group(0) @binding(1) var<storage, read> pos: array<Pos3>;
@group(0) @binding(2) var<storage, read> species: array<u32>;
@group(0) @binding(3) var<storage, read> ids: array<u32>;
@group(0) @binding(4) var<storage, read> vel: array<I3>;
@group(0) @binding(5) var<storage, read> values: array<f32>;

struct Pos3 {
    x: vec2<u32>,
    y: vec2<u32>,
    z: vec2<u32>,
}

struct I3 {
    x: i32,
    y: i32,
    z: i32,
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

fn ramp(t: f32) -> vec4<f32> {
    if (R.mid_color.a > 0.0) {
        return select(mix(R.mid_color, R.hot_color, 2.0 * t - 1.0), mix(R.cool_color, R.mid_color, 2.0 * t), t < 0.5);
    }
    return mix(R.cool_color, R.hot_color, t);
}

struct VOut {
    @builtin(position) clip: vec4<f32>,
    // The quad point and the sphere's centre in view space, and its radius.
    @location(0) q: vec3<f32>,
    @location(1) @interpolate(flat) center: vec3<f32>,
    @location(2) @interpolate(flat) radius: f32,
    @location(3) @interpolate(flat) color: vec4<f32>,
}

override highlight_only: bool = false;

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
    var out: VOut;
    let p = pos[ii];
    let s = species[ii];
    let world = vec3<f32>(coord(p.x), coord(p.y), coord(p.z));
    let c = (R.view * vec4<f32>(world, 1.0)).xyz;
    var r = max(R.radii[s], R.box_size.w);
    var color = R.colors[s];
    if (R.color_mode == 1u) {
        let v = vec3<f32>(f32(vel[ii].x), f32(vel[ii].y), f32(vel[ii].z));
        color = ramp(clamp(length(v) * R.speed_scale, 0.0, 1.0));
    } else if (R.color_mode == 2u) {
        let t = values[ids[ii]];
        // Negative values hide the particle (pages use it to show only part of the system).
        if (t < 0.0) {
            out.clip = vec4<f32>(2.0, 2.0, 2.0, 1.0);
            return out;
        }
        color = ramp(clamp(t, 0.0, 1.0));
    }
    if (highlight_only) {
        if (ids[ii] != R.highlight) {
            out.clip = vec4<f32>(2.0, 2.0, 2.0, 1.0);
            return out;
        }
        r = r * 1.6;
        color = R.highlight_color;
    } else if (c.z < R.slab.x || c.z > R.slab.y) {
        out.clip = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        return out;
    }
    // A quad facing the camera, a little larger than the sphere (perspective widens it), on the
    // sphere's front plane.
    let k = corner(vi);
    let q = c + vec3<f32>(k * r * 1.25, r);
    out.clip = R.proj * vec4<f32>(q, 1.0);
    out.q = q;
    out.center = c;
    out.radius = r;
    out.color = color;
    return out;
}

struct FOut {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

@fragment
fn fs(in: VOut) -> FOut {
    // The ray from the camera through this pixel, against the sphere.
    let d = normalize(in.q);
    let b = dot(d, in.center);
    let disc = b * b - dot(in.center, in.center) + in.radius * in.radius;
    if (disc < 0.0) {
        discard;
    }
    let h = d * (b - sqrt(disc));
    let n = (h - in.center) / in.radius;
    // A key light from the upper left, a little fill, and a highlight.
    let l = normalize(vec3<f32>(-0.45, 0.65, 0.6));
    let diffuse = max(dot(n, l), 0.0);
    let spec = pow(max(dot(reflect(-l, n), -d), 0.0), 24.0);
    var rgb = in.color.rgb * (0.32 + 0.72 * diffuse) + vec3<f32>(0.22 * spec);
    // Depth cue: further spheres fade a little toward the background.
    let fog = clamp((R.slab.w - h.z) / max(R.slab.w - R.slab.z, 1e-3), 0.0, 1.0);
    rgb = mix(rgb, R.background.rgb, 0.45 * fog);
    let clip = R.proj * vec4<f32>(h, 1.0);
    var out: FOut;
    out.color = vec4<f32>(rgb, 1.0);
    out.depth = clip.z / clip.w;
    return out;
}

// The box's twelve edges.
@vertex
fn vs_edges(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    // Each edge joins two corners that differ in one axis; corners are bit patterns (x, y, z).
    let e = vi / 2u;
    let axis = e / 4u;
    let other = e % 4u;
    // The two fixed bits of this edge, spread over the other two axes.
    var bits = vec3<u32>(0u, 0u, 0u);
    let a1 = (axis + 1u) % 3u;
    let a2 = (axis + 2u) % 3u;
    bits[a1] = other & 1u;
    bits[a2] = (other >> 1u) & 1u;
    bits[axis] = vi & 1u;
    let p = vec3<f32>(bits) * R.box_size.xyz;
    return R.proj * (R.view * vec4<f32>(p, 1.0));
}

@fragment
fn fs_edges() -> @location(0) vec4<f32> {
    return R.edge_color;
}
