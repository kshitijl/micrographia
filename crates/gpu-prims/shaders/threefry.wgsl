// Threefry-2x32 with 20 rounds (Salmon et al., SC11; Random123). Counter-based: the output is
// a pure function of (counter, key), so any thread can draw any random number.

fn rotl32(x: u32, r: u32) -> u32 {
    return (x << r) | (x >> (32u - r));
}

fn threefry2x32(ctr: vec2<u32>, key: vec2<u32>) -> vec2<u32> {
    var ks = array<u32, 3>(key.x, key.y, 0x1BD11BDAu ^ key.x ^ key.y);
    var rot = array<u32, 8>(13u, 15u, 26u, 6u, 17u, 29u, 16u, 24u);
    var x0 = ctr.x + ks[0];
    var x1 = ctr.y + ks[1];
    for (var r = 0u; r < 20u; r++) {
        x0 += x1;
        x1 = rotl32(x1, rot[r % 8u]);
        x1 ^= x0;
        if ((r & 3u) == 3u) {
            let s = (r >> 2u) + 1u;
            x0 += ks[s % 3u];
            x1 += ks[(s + 1u) % 3u] + s;
        }
    }
    return vec2<u32>(x0, x1);
}
