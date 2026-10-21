//! Record a scenario as a video, natively and headless: at any resolution and any number of
//! steps per frame, however long it takes, with the pages' renderer, piped to ffmpeg.
//!
//! `cargo xtask record <scenario> [--opt key=value]... [--size 1920x1080] [--frames 600]
//!  [--steps 50] [--fps 30] [--speed v_max] [--view x0,y0,x1,y1] [--colors hex,hex,...] [--out video.mp4]`
//!
//! `--opt` sets a scenario option (as a page's `options` would); `--speed` colours by speed,
//! saturating at v_max σ/τ; `--view` shows only that region of the box (σ); `--colors` gives
//! species 0, 1, 2, 3 their colours (default the pages' blue, red, cream, coffee).

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use pollster::block_on;
use std::io::Write;
use std::process::{Command, Stdio};

struct Args {
    scenario: String,
    opts: scenarios::Options,
    size: [u32; 2],
    frames: usize,
    steps: usize,
    fps: u32,
    speed: f64,
    view: Option<[f32; 4]>,
    colors: [[f32; 4]; 4],
    out: String,
}

fn parse() -> Result<Args, String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut a = Args { scenario: String::new(), opts: scenarios::Options::default(), size: [1280, 720], frames: 300, steps: 50, fps: 30, speed: 0.0, view: None, colors: COLORS, out: String::new() };
    let mut i = 0;
    let next = |i: &mut usize| -> Result<String, String> {
        *i += 1;
        argv.get(*i).cloned().ok_or_else(|| format!("{} needs a value", argv[*i - 1]))
    };
    while i < argv.len() {
        match argv[i].as_str() {
            "--opt" => {
                let kv = next(&mut i)?;
                let (k, v) = kv.split_once('=').ok_or("--opt takes key=value")?;
                a.opts = a.opts.with(k, v.parse().map_err(|_| format!("bad number in {kv}"))?);
            }
            "--size" => {
                let s = next(&mut i)?;
                let (w, h) = s.split_once('x').ok_or("--size takes WxH")?;
                a.size = [w.parse().map_err(|_| "bad width")?, h.parse().map_err(|_| "bad height")?];
            }
            "--frames" => a.frames = next(&mut i)?.parse().map_err(|_| "bad --frames")?,
            "--steps" => a.steps = next(&mut i)?.parse().map_err(|_| "bad --steps")?,
            "--fps" => a.fps = next(&mut i)?.parse().map_err(|_| "bad --fps")?,
            "--speed" => a.speed = next(&mut i)?.parse().map_err(|_| "bad --speed")?,
            "--view" => {
                let v: Vec<f32> = next(&mut i)?.split(',').map(|x| x.parse().unwrap_or(0.0)).collect();
                if v.len() != 4 {
                    return Err("--view takes x0,y0,x1,y1".into());
                }
                a.view = Some([v[0], v[1], v[2], v[3]]);
            }
            "--colors" => {
                for (k, h) in next(&mut i)?.split(',').take(4).enumerate() {
                    a.colors[k] = rgb(u32::from_str_radix(h.trim_start_matches('#'), 16).map_err(|_| format!("bad colour {h}"))?);
                }
            }
            "--out" => a.out = next(&mut i)?,
            s if s.starts_with("--") => return Err(format!("unknown option {s}")),
            s => a.scenario = s.to_string(),
        }
        i += 1;
    }
    if a.scenario.is_empty() {
        return Err(format!("which scenario? one of: {}", scenarios::NAMES.join(", ")));
    }
    if a.out.is_empty() {
        a.out = format!("{}.mp4", a.scenario);
    }
    Ok(a)
}

/// The pages' colours (style.css): species-2 (blue), species-3 (red), cream, coffee; and the
/// stage's background.
const COLORS: [[f32; 4]; 4] = [rgb(0x4f7cc4), rgb(0xc9503c), rgb(0xf3e4c6), rgb(0x5b3a26)];
const BACKGROUND: [f64; 3] = [0x1d as f64 / 255.0, 0x17 as f64 / 255.0, 0x12 as f64 / 255.0];

const fn rgb(c: u32) -> [f32; 4] {
    [((c >> 16) & 255) as f32 / 255.0, ((c >> 8) & 255) as f32 / 255.0, (c & 255) as f32 / 255.0, 1.0]
}

fn main() {
    let a = match parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}\n\n{}", include_str!("main.rs").lines().take(10).map(|l| l.trim_start_matches("//!")).collect::<Vec<_>>().join("\n"));
            std::process::exit(2);
        }
    };
    let gpu = block_on(Gpu::new()).expect("a GPU");
    let prims = Prims::new(&gpu);
    let sc = scenarios::build(&a.scenario, &a.opts).unwrap_or_else(|e| panic!("{e}"));
    let mut engine = GpuEngine::new(&prims, sc.sys.clone(), &sc.state);
    // As a page starts it: the warm-up, then the finishing touch (a temperature, a marked disc).
    if sc.warmup > 0 || sc.finish != scenarios::Finish::None {
        engine.step(sc.warmup);
        let mut state = block_on(engine.read_state());
        sc.finish.apply(&mut state, &sc.sys);
        engine.upload(&state);
    }
    let [w, h] = a.size;
    let mut r = render::Renderer::offscreen(&gpu, &engine, &sc.radii, w, h);
    r.params.colors = a.colors;
    r.background = wgpu::Color { r: BACKGROUND[0], g: BACKGROUND[1], b: BACKGROUND[2], a: 1.0 };
    if a.speed > 0.0 {
        r.params.speed_scale = (1.0 / (a.speed * engine.sys.dt * md_core::ONE as f64)) as f32;
    }
    let b = engine.sys.box_f64();
    let v = a.view.unwrap_or([0.0, 0.0, b[0] as f32, b[1] as f32]);
    r.set_view([v[0], v[1]], [v[2], v[3]]);

    let mut ffmpeg = Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &format!("{w}x{h}"), "-r", &a.fps.to_string(), "-i", "-"])
        .args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "18", &a.out])
        .stdin(Stdio::piped())
        .spawn()
        .expect("ffmpeg (brew install ffmpeg)");
    let mut pipe = ffmpeg.stdin.take().unwrap();
    let t0 = std::time::Instant::now();
    for f in 0..a.frames {
        if f > 0 {
            engine.step(a.steps);
        }
        r.render();
        let frame = block_on(r.read_frame());
        pipe.write_all(&frame).expect("writing to ffmpeg");
        if f % 50 == 49 {
            eprintln!("frame {}/{} ({:.0} s)", f + 1, a.frames, t0.elapsed().as_secs_f64());
        }
    }
    drop(pipe);
    let status = ffmpeg.wait().expect("ffmpeg");
    assert!(status.success(), "ffmpeg failed");
    let tau = a.frames as f64 * a.steps as f64 * engine.sys.dt;
    eprintln!("wrote {} ({} frames, {w}×{h}, {} particles, {tau:.0} τ, in {:.0} s)", a.out, a.frames, engine.n(), t0.elapsed().as_secs_f64());
}
