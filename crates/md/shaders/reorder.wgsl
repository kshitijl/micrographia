// Spatial reordering: permute particle storage into Morton order of cells, so particles that
// are close in space are close in memory. Requires params.wgsl.
// Particle identity travels in `ids`; results never depend on storage order.

@group(0) @binding(1) var<storage, read_write> pos: array<vec4<u32>>;
@group(0) @binding(2) var<storage, read_write> vel: array<vec2<i32>>;
@group(0) @binding(3) var<storage, read_write> species: array<u32>;
@group(0) @binding(4) var<storage, read_write> ids: array<u32>;
@group(0) @binding(5) var<storage, read_write> keys: array<u32>;
@group(0) @binding(6) var<storage, read> perm: array<u32>;
@group(0) @binding(7) var<storage, read_write> tmp_a: array<vec4<u32>>;
@group(0) @binding(8) var<storage, read_write> tmp_b: array<vec2<i32>>;
@group(0) @binding(9) var<storage, read_write> tmp_c: array<u32>;
@group(0) @binding(10) var<storage, read_write> tmp_d: array<u32>;

@compute @workgroup_size(256)
fn morton_keys(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= P.n) {
        return;
    }
    let p = pos[i];
    let cx = min(u32(coord_f32(p.xy) * P.cell_scale.x), P.ncx - 1u);
    let cy = min(u32(coord_f32(p.zw) * P.cell_scale.y), P.ncy - 1u);
    keys[i] = morton(cx, cy);
}

@compute @workgroup_size(256)
fn gather_motion(@builtin(global_invocation_id) gid: vec3<u32>) {
    let k = gid.x;
    if (k >= P.n) {
        return;
    }
    let i = perm[k];
    tmp_a[k] = pos[i];
    tmp_b[k] = vel[i];
}

@compute @workgroup_size(256)
fn gather_meta(@builtin(global_invocation_id) gid: vec3<u32>) {
    let k = gid.x;
    if (k >= P.n) {
        return;
    }
    let i = perm[k];
    tmp_c[k] = species[i];
    tmp_d[k] = ids[i];
}

@compute @workgroup_size(256)
fn copy_back_motion(@builtin(global_invocation_id) gid: vec3<u32>) {
    let k = gid.x;
    if (k >= P.n) {
        return;
    }
    pos[k] = tmp_a[k];
    vel[k] = tmp_b[k];
}

@compute @workgroup_size(256)
fn copy_back_meta(@builtin(global_invocation_id) gid: vec3<u32>) {
    let k = gid.x;
    if (k >= P.n) {
        return;
    }
    species[k] = tmp_c[k];
    ids[k] = tmp_d[k];
}
