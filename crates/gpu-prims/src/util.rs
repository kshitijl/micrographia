//! Small helpers for buffers, shaders, pipelines and asynchronous readback.

use crate::Gpu;
use wgpu::util::DeviceExt;

/// Threads per workgroup for every kernel in the project (the WebGPU default maximum).
pub const WG: u32 = 256;

/// Number of workgroups needed to cover `n` items at `per_group` items per workgroup.
pub fn groups(n: u32, per_group: u32) -> u32 {
    n.div_ceil(per_group).max(1)
}

pub const STORAGE: wgpu::BufferUsages = wgpu::BufferUsages::STORAGE
    .union(wgpu::BufferUsages::COPY_SRC)
    .union(wgpu::BufferUsages::COPY_DST);

/// A zeroed storage buffer of `size` bytes (rounded up to 4, at least 4).
pub fn storage(gpu: &Gpu, label: &str, size: u64) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size.max(4).next_multiple_of(4),
        usage: STORAGE,
        mapped_at_creation: false,
    })
}

/// A storage buffer initialized from `data` (at least 4 bytes).
pub fn storage_init<T: bytemuck::NoUninit>(gpu: &Gpu, label: &str, data: &[T]) -> wgpu::Buffer {
    let bytes: &[u8] = bytemuck::cast_slice(data);
    if bytes.is_empty() {
        return storage(gpu, label, 4);
    }
    gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytes,
        usage: STORAGE,
    })
}

pub fn uniform_init<T: bytemuck::NoUninit>(gpu: &Gpu, label: &str, data: &T) -> wgpu::Buffer {
    gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::bytes_of(data),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}

pub fn shader(gpu: &Gpu, label: &str, src: &str) -> wgpu::ShaderModule {
    gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(src.into()),
    })
}

/// A compute pipeline with an automatically derived layout.
pub fn pipeline(gpu: &Gpu, module: &wgpu::ShaderModule, entry: &str) -> wgpu::ComputePipeline {
    gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(entry),
        layout: None,
        module,
        entry_point: Some(entry),
        compilation_options: Default::default(),
        cache: None,
    })
}

/// Bind `buffers` to bindings 0, 1, 2, ... of group 0 of `pipeline`.
pub fn bind(gpu: &Gpu, pipeline: &wgpu::ComputePipeline, buffers: &[&wgpu::Buffer]) -> wgpu::BindGroup {
    let numbered: Vec<(u32, &wgpu::Buffer)> = buffers.iter().enumerate().map(|(i, b)| (i as u32, *b)).collect();
    bind_at(gpu, pipeline, &numbered)
}

/// Bind buffers to explicit binding numbers, for entry points that use only some of a module's
/// bindings (automatic layouts contain only the bindings an entry point uses).
pub fn bind_at(gpu: &Gpu, pipeline: &wgpu::ComputePipeline, buffers: &[(u32, &wgpu::Buffer)]) -> wgpu::BindGroup {
    let entries: Vec<wgpu::BindGroupEntry> = buffers
        .iter()
        .map(|(i, b)| wgpu::BindGroupEntry { binding: *i, resource: b.as_entire_binding() })
        .collect();
    gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &entries,
    })
}

/// One recorded dispatch: a pipeline, its bind group and a workgroup count.
#[derive(Clone)]
pub struct Dispatch {
    pub pipeline: wgpu::ComputePipeline,
    pub bind_group: wgpu::BindGroup,
    pub groups: u32,
}

impl Dispatch {
    pub fn new(gpu: &Gpu, pipeline: &wgpu::ComputePipeline, buffers: &[&wgpu::Buffer], groups: u32) -> Self {
        Self { pipeline: pipeline.clone(), bind_group: bind(gpu, pipeline, buffers), groups }
    }

    pub fn new_at(gpu: &Gpu, pipeline: &wgpu::ComputePipeline, buffers: &[(u32, &wgpu::Buffer)], groups: u32) -> Self {
        Self { pipeline: pipeline.clone(), bind_group: bind_at(gpu, pipeline, buffers), groups }
    }

    pub fn encode(&self, pass: &mut wgpu::ComputePass<'_>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.dispatch_workgroups(self.groups, 1, 1);
    }
}

/// Encode a list of dispatches into a single compute pass and submit.
pub fn run(gpu: &Gpu, dispatches: &[&Dispatch]) {
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    {
        let mut pass = enc.begin_compute_pass(&Default::default());
        for d in dispatches {
            d.encode(&mut pass);
        }
    }
    gpu.queue.submit([enc.finish()]);
}

/// Copy `size` bytes of `buffer` (from `offset`) back to the host.
///
/// Asynchronous everywhere: in the browser the mapping completes on the event loop; natively
/// the device is polled until it completes.
pub async fn read_bytes(gpu: &Gpu, buffer: &wgpu::Buffer, offset: u64, size: u64) -> Vec<u8> {
    let size = size.next_multiple_of(4);
    let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: size.max(4),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    enc.copy_buffer_to_buffer(buffer, offset, &staging, 0, size);
    gpu.queue.submit([enc.finish()]);
    let (tx, rx) = futures_channel::oneshot::channel();
    staging.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    gpu.wait();
    rx.await.expect("map callback dropped").expect("buffer map failed");
    let data = staging.slice(..).get_mapped_range().expect("mapped range").to_vec();
    staging.unmap();
    data
}

pub async fn read<T: bytemuck::AnyBitPattern + bytemuck::NoUninit>(gpu: &Gpu, buffer: &wgpu::Buffer, n: usize) -> Vec<T> {
    let bytes = read_bytes(gpu, buffer, 0, (n * std::mem::size_of::<T>()) as u64).await;
    bytemuck::pod_collect_to_vec(&bytes[..n * std::mem::size_of::<T>()])
}

/// Replace `{{KEY}}` placeholders in a shader template.
pub fn template(src: &str, subs: &[(&str, &str)]) -> String {
    let mut s = src.to_string();
    for (k, v) in subs {
        s = s.replace(&format!("{{{{{k}}}}}"), v);
    }
    assert!(!s.contains("{{"), "unsubstituted placeholder in shader template");
    s
}
