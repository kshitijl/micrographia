//! Project commands: `cargo xtask <command>`.
//!
//!   test           native tests: CPU engines, GPU primitives and engines (needs a GPU)
//!   wasm           build the browser module into web/src/wasm
//!   web            wasm + production build of the pages into web/dist
//!   dev            wasm + Vite dev server
//!   test-browser   web + Playwright tests in headless Chrome
//!   bench [args]   native benchmarks (results appended to bench/results.jsonl)
//!   all            test + test-browser

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn run(cmd: &mut Command) -> Result<(), String> {
    eprintln!("$ {cmd:?}");
    let status = cmd.status().map_err(|e| format!("could not start {cmd:?}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{cmd:?} failed with {status}")) }
}

fn cargo() -> Command {
    let mut c = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    c.current_dir(root());
    c
}

fn npx(args: &[&str]) -> Command {
    let mut c = Command::new("npx");
    c.args(args).current_dir(root().join("web"));
    c
}

fn ensure_node_modules() -> Result<(), String> {
    if !root().join("web/node_modules").exists() {
        run(Command::new("npm").arg("ci").current_dir(root().join("web")))?;
    }
    Ok(())
}

fn wasm() -> Result<(), String> {
    run(cargo().args(["build", "-p", "sm-wasm", "--target", "wasm32-unknown-unknown", "--profile", "wasm"]))?;
    let wasm = root().join("target/wasm32-unknown-unknown/wasm/sm_wasm.wasm");
    run(Command::new("wasm-bindgen")
        .args(["--target", "web", "--out-dir"])
        .arg(root().join("web/src/wasm"))
        .arg(wasm))
}

fn web() -> Result<(), String> {
    wasm()?;
    ensure_node_modules()?;
    run(&mut npx(&["tsc", "--noEmit", "-p", "."]))?;
    run(&mut npx(&["vite", "build"]))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest: Vec<&str> = args.iter().skip(1).map(|s| s.as_str()).collect();
    let result = match args.first().map(|s| s.as_str()) {
        Some("test") => run(cargo().args(["test", "--workspace"]).args(&rest)),
        Some("wasm") => wasm(),
        Some("web") => web(),
        Some("dev") => wasm().and_then(|_| ensure_node_modules()).and_then(|_| run(&mut npx(&["vite"]))),
        Some("test-browser") => web().and_then(|_| run(npx(&["playwright", "test"]).args(&rest))),
        Some("bench") => run(cargo().args(["run", "--release", "-p", "bench", "--"]).args(&rest)),
        Some("all") => run(cargo().args(["test", "--workspace"])).and_then(|_| web()).and_then(|_| run(&mut npx(&["playwright", "test"]))),
        _ => {
            eprintln!("{}", include_str!("main.rs").lines().take(10).map(|l| l.trim_start_matches("//!")).collect::<Vec<_>>().join("\n"));
            return ExitCode::FAILURE;
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask: {e}");
            ExitCode::FAILURE
        }
    }
}
