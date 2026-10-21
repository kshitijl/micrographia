// Browser only: natively this crate is empty, so `cargo test --workspace` skips it.
#![cfg(target_arch = "wasm32")]

//! The browser API: simulations for pages, and the self-tests that run the native test suites'
//! GPU checks under the browser's WebGPU implementation.

mod render;
mod render3;
mod sims;
mod sims3;

pub use sims::*;
pub use sims3::*;

use gpu_prims::{Gpu, Prims};
use js_sys::{Array, Object, Reflect};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

pub(crate) fn obj(pairs: &[(&str, JsValue)]) -> JsValue {
    let o = Object::new();
    for (k, v) in pairs {
        Reflect::set(&o, &JsValue::from_str(k), v).expect("set property");
    }
    o.into()
}

pub(crate) fn err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// Create a device on the browser's WebGPU, compatible with `canvas` if given.
pub(crate) async fn browser_gpu(canvas: Option<web_sys::HtmlCanvasElement>) -> Result<(Gpu, Option<wgpu::Surface<'static>>), JsError> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::BROWSER_WEBGPU;
    let instance = wgpu::Instance::new(desc);
    let surface = match canvas {
        Some(c) => Some(instance.create_surface(wgpu::SurfaceTarget::Canvas(c)).map_err(err)?),
        None => None,
    };
    let gpu = Gpu::from_instance(&instance, surface.as_ref()).await.map_err(err)?;
    Ok((gpu, surface))
}

/// Run every GPU self-check; resolves to `[{name, ok, message, ms}]`.
#[wasm_bindgen]
pub async fn selftest() -> Result<JsValue, JsError> {
    let (gpu, _) = browser_gpu(None).await?;
    let prims = Prims::new(&gpu);
    let results = Array::new();
    let names = gpu_prims::checks::NAMES.iter().map(|n| (true, *n)).chain(md::checks::NAMES.iter().map(|n| (false, *n)));
    for (is_prim, name) in names {
        let t0 = js_sys::Date::now();
        let r = if is_prim { gpu_prims::checks::run(name, &prims).await } else { md::checks::run(name, &prims).await };
        let ms = js_sys::Date::now() - t0;
        let (ok, message) = match r {
            Ok(m) => (true, m),
            Err(m) => (false, m),
        };
        results.push(&obj(&[
            ("name", name.into()),
            ("ok", ok.into()),
            ("message", message.into()),
            ("ms", ms.into()),
        ]));
    }
    Ok(obj(&[("adapter", gpu.describe().into()), ("results", results.into())]))
}
