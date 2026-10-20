//! GPU timestamp queries around compute passes, for benchmarks (needs `TIMESTAMP_QUERY`).

use crate::Gpu;

pub struct PassTimer {
    gpu: Gpu,
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    capacity: u32,
    used: u32,
}

impl PassTimer {
    /// A timer for up to `passes` timed passes, or `None` if the device has no timestamps.
    pub fn new(gpu: &Gpu, passes: u32) -> Option<Self> {
        if !gpu.has_timestamps() {
            return None;
        }
        let count = passes * 2;
        let set = gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("pass timer"),
            ty: wgpu::QueryType::Timestamp,
            count,
        });
        let resolve = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("timestamps"),
            size: count as u64 * 8,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        Some(Self { gpu: gpu.clone(), set, resolve, capacity: passes, used: 0 })
    }

    /// Timestamp writes for the next pass.
    pub fn next(&mut self) -> wgpu::ComputePassTimestampWrites<'_> {
        assert!(self.used < self.capacity, "PassTimer capacity exceeded");
        let i = self.used;
        self.used += 1;
        wgpu::ComputePassTimestampWrites {
            query_set: &self.set,
            beginning_of_pass_write_index: Some(2 * i),
            end_of_pass_write_index: Some(2 * i + 1),
        }
    }

    pub fn resolve(&self, enc: &mut wgpu::CommandEncoder) {
        if self.used > 0 {
            enc.resolve_query_set(&self.set, 0..2 * self.used, &self.resolve, 0);
        }
    }

    /// Durations of the timed passes in nanoseconds, in the order they were requested.
    pub async fn read_ns(&mut self) -> Vec<f64> {
        let n = self.used as usize;
        self.used = 0;
        if n == 0 {
            return Vec::new();
        }
        let ticks = crate::util::read_bytes(&self.gpu, &self.resolve, 0, n as u64 * 16).await;
        let ticks: Vec<u64> = bytemuck::pod_collect_to_vec(&ticks);
        let period = self.gpu.queue.get_timestamp_period() as f64;
        (0..n).map(|i| ticks[2 * i + 1].saturating_sub(ticks[2 * i]) as f64 * period).collect()
    }
}
