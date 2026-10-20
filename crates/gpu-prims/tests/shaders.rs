//! Parse and validate every shader with naga, without a GPU.

#[test]
fn shaders_validate() {
    for (name, src) in gpu_prims::prims::shader_sources() {
        let module = naga::front::wgsl::parse_str(&src).unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(&src)));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
            .validate(&module)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
    }
}
