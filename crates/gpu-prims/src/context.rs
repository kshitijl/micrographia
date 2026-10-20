//! Device creation shared by native tests, benchmarks and the browser.

use std::sync::Arc;

/// A device and queue, cheap to clone.
#[derive(Clone)]
pub struct Gpu {
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: Arc<wgpu::AdapterInfo>,
    pub features: wgpu::Features,
}

#[derive(Debug)]
pub struct GpuError(pub String);

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GpuError {}

/// Features we use when the adapter has them. Everything else must work without them.
pub fn optional_features() -> wgpu::Features {
    wgpu::Features::TIMESTAMP_QUERY
}

impl Gpu {
    /// Create a device on the default adapter, without a surface.
    pub async fn new() -> Result<Self, GpuError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        Self::from_instance(&instance, None).await
    }

    /// Create a device on an adapter compatible with `surface` (if given).
    pub async fn from_instance(
        instance: &wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self, GpuError> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: surface,
                ..Default::default()
            })
            .await
            .map_err(|e| GpuError(format!("no GPU adapter: {e}")))?;
        let features = adapter.features() & optional_features();
        // WebGPU default limits, so native runs exercise the same constraints as the browser.
        let limits = wgpu::Limits::default();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("sm-device"),
                required_features: features,
                required_limits: limits,
                ..Default::default()
            })
            .await
            .map_err(|e| GpuError(format!("request_device failed: {e}")))?;
        Ok(Self { info: Arc::new(adapter.get_info()), adapter, device, queue, features })
    }

    pub fn has_timestamps(&self) -> bool {
        self.features.contains(wgpu::Features::TIMESTAMP_QUERY)
    }

    /// Block until submitted work is done (native only; a no-op in the browser).
    pub fn wait(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        }
    }

    pub fn describe(&self) -> String {
        format!("{} ({:?}, {:?})", self.info.name, self.info.backend, self.info.device_type)
    }
}
