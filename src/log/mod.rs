use crate::config::Config;
use crate::wire::{LlmRequest, LlmResponse};
use fs_err as fs;
use serde_json::to_string_pretty;
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct SavedPaths {
    pub dir: PathBuf,
    pub request: Option<PathBuf>,
    pub response: Option<PathBuf>,
}

fn tx_dir(root: &Path, tx: Uuid) -> PathBuf {
    root.join(".vibe").join("tx").join(tx.to_string())
}

pub fn save_stage(
    stage: &str,
    req: &LlmRequest,
    resp: &LlmResponse,
    tx: Uuid,
    cfg: &Config,
    save_request: bool,
    save_response: bool,
) -> anyhow::Result<SavedPaths> {
    let dir = tx_dir(Path::new(&cfg.root), tx);
    fs::create_dir_all(&dir)?;

    let mut request_path = None;
    let mut response_path = None;

    if save_request {
        let p = dir.join(format!("{stage}.request.json"));
        fs::write(&p, to_string_pretty(req)?)?;
        request_path = Some(p);
    }

    if save_response {
        let p = dir.join(format!("{stage}.response.json"));
        fs::write(&p, to_string_pretty(resp)?)?;
        response_path = Some(p);
    }

    Ok(SavedPaths { dir, request: request_path, response: response_path })
}

pub fn print_planned_paths(root: &Path, tx: Uuid) {
    let dir = tx_dir(root, tx);
    println!("debug: planned artifacts directory: {}", dir.display());
    println!("debug: planned request path: {}", dir.join("plan.request.json").display());
    println!("debug: planned response path: {}", dir.join("plan.response.json").display());
    std::io::stdout().flush().ok();
}

pub fn print_saved_paths(stage: &str, saved: &SavedPaths) {
    println!("debug[{stage}]: artifacts directory: {}", saved.dir.display());
    if let Some(p) = &saved.request {
        println!("debug[{stage}]: request saved at: {}", p.display());
    } else {
        println!("debug[{stage}]: request not saved (flag off)");
    }
    if let Some(p) = &saved.response {
        println!("debug[{stage}]: response saved at: {}", p.display());
    } else {
        println!("debug[{stage}]: response not saved (flag off)");
    }
    std::io::stdout().flush().ok();
}

pub fn print_json_debug(stage: &str, req: &LlmRequest, resp: &LlmResponse) -> anyhow::Result<()> {
    let req_json = to_string_pretty(req)?;
    let resp_json = to_string_pretty(resp)?;
    eprintln!("\n===== DEBUG [{stage}]: REQUEST JSON =====\n{}\n", req_json);
    eprintln!("===== DEBUG [{stage}]: RESPONSE JSON =====\n{}\n", resp_json);
    std::io::stderr().flush().ok();
    Ok(())
}
