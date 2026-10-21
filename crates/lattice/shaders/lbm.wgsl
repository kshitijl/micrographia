// Single-component Shan–Chen lattice Boltzmann (D2Q9), with the interaction potential taken from
// the Carnahan–Starling equation of state at each site's temperature (Yuan & Schaefer, Phys.
// Fluids 18, 042101, 2006), the exact-difference forcing (Kupershtokh et al., Comput. Math. Appl.
// 58, 965, 2009), and optionally a temperature field carried by the flow, with the heating and
// cooling of compression and expansion (Gong & Cheng, Int. J. Heat Mass Transfer 55, 4923,
// 2012). CPU twin of the equation of state: lattice::lbm::Eos.
//
// Lattice units: Δx = Δt = 1, c_s² = 1/3. Sites are row-major from the bottom row; rows 0 and
// h − 1 are solid walls (bounce-back), and x is periodic.

struct LbmParams {
    w: u32,
    h: u32,
    n: u32,
    // 1: evolve the temperature field; 0: keep it as set.
    thermal: u32,
    // 1/τ for the populations (τ = 1: kinematic viscosity 1/6).
    omega: f32,
    // Downward acceleration.
    gravity: f32,
    // Carnahan–Starling: p = ρT (1 + η + η² − η³)/(1 − η)³ − aρ², η = bρ/4 (R = 1).
    a: f32,
    b: f32,
    // Wall temperatures: the floor (a hot plate, over x in [plate_x0, plate_x1)) and the ceiling
    // (≤ 0: insulating).
    t_floor: f32,
    t_ceiling: f32,
    plate_x0: u32,
    plate_x1: u32,
    // Thermal diffusivity and heat capacity per unit mass.
    chi: f32,
    cv: f32,
    // ψ of wall sites, relative to the neighbouring fluid's (1: neutral, < 1: the liquid avoids
    // the wall, > 1 wets it).
    wetting: f32,
    _p: u32,
}

@group(0) @binding(0) var<uniform> P: LbmParams;
@group(0) @binding(1) var<storage, read> f_in: array<f32>;
@group(0) @binding(2) var<storage, read_write> f_out: array<f32>;
@group(0) @binding(3) var<storage, read_write> rho: array<f32>;
@group(0) @binding(4) var<storage, read_write> psi: array<f32>;
// Velocity (x, y) per site, after the force (the physical velocity).
@group(0) @binding(5) var<storage, read_write> vel: array<vec2<f32>>;
@group(0) @binding(6) var<storage, read> temp_in: array<f32>;
@group(0) @binding(7) var<storage, read_write> temp_out: array<f32>;

var<private> EX: array<i32, 9> = array<i32, 9>(0, 1, 0, -1, 0, 1, -1, -1, 1);
var<private> EY: array<i32, 9> = array<i32, 9>(0, 0, 1, 0, -1, 1, 1, -1, -1);
var<private> WT: array<f32, 9> = array<f32, 9>(4.0 / 9.0, 1.0 / 9.0, 1.0 / 9.0, 1.0 / 9.0, 1.0 / 9.0, 1.0 / 36.0, 1.0 / 36.0, 1.0 / 36.0, 1.0 / 36.0);
var<private> OPP: array<u32, 9> = array<u32, 9>(0u, 3u, 4u, 1u, 2u, 7u, 8u, 5u, 6u);

fn is_wall(y: u32) -> bool {
    return y == 0u || y == P.h - 1u;
}

// The Carnahan–Starling factor (1 + η + η² − η³)/(1 − η)³.
fn cs(r: f32) -> f32 {
    let e = P.b * r / 4.0;
    let m = 1.0 - e;
    return (1.0 + e + e * e - e * e * e) / (m * m * m);
}

fn pressure(r: f32, t: f32) -> f32 {
    return r * t * cs(r) - P.a * r * r;
}

// ψ with p = c_s² ρ + (G c_s²/2) ψ² and G = −1: ψ = √(6 (ρ/3 − p)) (clamped at 0 where the EOS
// pressure exceeds the ideal gas's, which does not happen in the liquid–vapour range).
fn psi_of(r: f32, t: f32) -> f32 {
    return sqrt(max(0.0, 2.0 * r - 6.0 * pressure(r, t)));
}

fn site(x: u32, y: u32) -> u32 {
    return y * P.w + x;
}

// Density and ψ at every site (walls: ψ of the fluid just inside, times the wetting factor).
@compute @workgroup_size(256)
fn macro_fields(@builtin(global_invocation_id) gid: vec3<u32>) {
    let s = gid.x;
    if (s >= P.n) {
        return;
    }
    let y = s / P.w;
    if (is_wall(y)) {
        rho[s] = 0.0;
        return;
    }
    var r = 0.0;
    for (var i = 0u; i < 9u; i++) {
        r += f_in[i * P.n + s];
    }
    rho[s] = r;
    psi[s] = psi_of(r, temp_in[s]);
}

// Walls take ψ from the fluid next to them, scaled by the wetting factor.
@compute @workgroup_size(256)
fn wall_psi(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    if (x >= P.w) {
        return;
    }
    psi[site(x, 0u)] = P.wetting * psi[site(x, 1u)];
    psi[site(x, P.h - 1u)] = P.wetting * psi[site(x, P.h - 2u)];
}

fn feq(i: u32, r: f32, u: vec2<f32>) -> f32 {
    let eu = f32(EX[i]) * u.x + f32(EY[i]) * u.y;
    return WT[i] * r * (1.0 + 3.0 * eu + 4.5 * eu * eu - 1.5 * dot(u, u));
}

// Collide (BGK, exact-difference forcing with the Shan–Chen force and gravity) and push each
// population to its neighbour; populations heading into a wall bounce back.
@compute @workgroup_size(256)
fn collide_stream(@builtin(global_invocation_id) gid: vec3<u32>) {
    let s = gid.x;
    if (s >= P.n) {
        return;
    }
    let x = s % P.w;
    let y = s / P.w;
    if (is_wall(y)) {
        return;
    }
    var f: array<f32, 9>;
    var r = 0.0;
    var m = vec2<f32>(0.0, 0.0);
    for (var i = 0u; i < 9u; i++) {
        let v = f_in[i * P.n + s];
        f[i] = v;
        r += v;
        m += v * vec2<f32>(f32(EX[i]), f32(EY[i]));
    }
    // Shan–Chen: F = −G ψ(x) Σ w_i ψ(x + e_i) e_i, G = −1.
    var sum = vec2<f32>(0.0, 0.0);
    for (var i = 1u; i < 9u; i++) {
        let xn = u32((i32(x) + EX[i] + i32(P.w)) % i32(P.w));
        let yn = u32(i32(y) + EY[i]);
        sum += WT[i] * psi[site(xn, yn)] * vec2<f32>(f32(EX[i]), f32(EY[i]));
    }
    let force = psi[s] * sum - vec2<f32>(0.0, r * P.gravity);
    let u = m / r;
    let du = force / r;
    vel[s] = u + 0.5 * du;
    for (var i = 0u; i < 9u; i++) {
        let e0 = feq(i, r, u);
        let post = f[i] - P.omega * (f[i] - e0) + feq(i, r, u + du) - e0;
        let xn = u32((i32(x) + EX[i] + i32(P.w)) % i32(P.w));
        let yn = u32(i32(y) + EY[i]);
        if (is_wall(yn)) {
            f_out[OPP[i] * P.n + s] = post;
        } else {
            f_out[i * P.n + site(xn, yn)] = post;
        }
    }
}

// The temperature field: carried by the flow, diffusing, and warmed or cooled by compression or
// expansion (−T/(ρ c_v) (∂p/∂T)_ρ ∇·u, which is what makes evaporation take heat). Explicit
// finite differences; the floor's plate and the ceiling are fixed temperatures, other walls
// insulate.
@compute @workgroup_size(256)
fn heat(@builtin(global_invocation_id) gid: vec3<u32>) {
    let s = gid.x;
    if (s >= P.n) {
        return;
    }
    let x = s % P.w;
    let y = s / P.w;
    let t = temp_in[s];
    if (is_wall(y) || P.thermal == 0u) {
        temp_out[s] = t;
        return;
    }
    let xl = (x + P.w - 1u) % P.w;
    let xr = (x + 1u) % P.w;
    var tb = temp_in[site(x, y - 1u)];
    if (y == 1u) {
        tb = select(t, P.t_floor, x >= P.plate_x0 && x < P.plate_x1);
    }
    var tt = temp_in[site(x, y + 1u)];
    if (y + 2u == P.h) {
        tt = select(t, P.t_ceiling, P.t_ceiling > 0.0);
    }
    let tl = temp_in[site(xl, y)];
    let tr = temp_in[site(xr, y)];
    let u = vel[s];
    let lap = tl + tr + tb + tt - 4.0 * t;
    let grad = vec2<f32>(0.5 * (tr - tl), 0.5 * (tt - tb));
    // ∇·u, with zero velocity in the walls.
    let ub = select(vel[site(x, y - 1u)].y, 0.0, y == 1u);
    let ut = select(vel[site(x, y + 1u)].y, 0.0, y + 2u == P.h);
    let div = 0.5 * (vel[site(xr, y)].x - vel[site(xl, y)].x) + 0.5 * (ut - ub);
    let r = rho[s];
    let dpdt = r * cs(r);
    temp_out[s] = t + P.chi * lap - dot(u, grad) - t * dpdt / (r * P.cv) * div;
}

// Set a disc of fluid (radius r around (cx, cy)) to density `density` at rest: a dropper adding
// a drop of liquid, or a bubble of vapour. Writes both population buffers.
struct Paint {
    cx: f32,
    cy: f32,
    r: f32,
    density: f32,
}
@group(0) @binding(8) var<uniform> B: Paint;
@group(0) @binding(9) var<storage, read_write> f_other: array<f32>;

@compute @workgroup_size(256)
fn paint(@builtin(global_invocation_id) gid: vec3<u32>) {
    let s = gid.x;
    if (s >= P.n) {
        return;
    }
    let x = f32(s % P.w) + 0.5;
    let y = f32(s / P.w) + 0.5;
    var dx = abs(x - B.cx);
    dx = min(dx, f32(P.w) - dx);
    let dy = y - B.cy;
    if (dx * dx + dy * dy > B.r * B.r || is_wall(s / P.w)) {
        return;
    }
    for (var i = 0u; i < 9u; i++) {
        let v = WT[i] * B.density;
        f_out[i * P.n + s] = v;
        f_other[i * P.n + s] = v;
    }
}
