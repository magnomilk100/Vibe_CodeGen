use crate::wire::FileBlob;
use fs_err as fs;
use std::path::{Path, PathBuf};

pub mod embeddings; // NEW: semantic-ish retrieval support

/// Read the first `max_bytes` of each given file (relative to `root`) and
/// produce FileBlob entries for the LLM request.
pub fn snapshot_files(paths: &[String], root: &Path, max_bytes: usize) -> Vec<FileBlob> {
    let mut out = Vec::new();
    for rel in paths {
        let abs = root.join(rel);
        if !abs.exists() || !abs.is_file() {
            continue;
        }
        match read_prefix(&abs, max_bytes) {
            Ok((content, bytes, truncated)) => out.push(FileBlob {
                path: rel.clone(),
                bytes,
                hash: None,
                truncated,
                content,
            }),
            Err(_) => {
                // best-effort skip
                continue;
            }
        }
    }
    out
}

fn read_prefix(path: &Path, max_bytes: usize) -> anyhow::Result<(String, usize, bool)> {
    let data = fs::read(path)?;
    let bytes = data.len();
    let truncated = bytes > max_bytes;
    let slice = if truncated { &data[..max_bytes] } else { &data[..] };
    let content = String::from_utf8_lossy(slice).into_owned();
    Ok((content, bytes, truncated))
}

/// Select relevant Next.js files for the current task, mixing:
/// - baseline App Router files
/// - package.json (always)
/// - top-k semantic-ish hits from embeddings.jsonl (if present)
///
/// `vibe_out` points to the `.vibe/out` directory. On any error/missing files,
/// we gracefully fall back to the baseline set.
pub fn select_relevant_files(task: &str, root: &Path, vibe_out: &Path, top_k: usize) -> Vec<String> {
    // Baseline set (kept for backward compatibility)
    let mut set = vec![
        "src/app/page.tsx".to_string(),
        "src/app/layout.tsx".to_string(),
        "src/app/components/InteractiveButton.tsx".to_string(),
        "package.json".to_string(),
    ];

    // Try to load the embedding index
    match embeddings::EmbeddingIndex::load(vibe_out) {
        Ok(index) => {
            // Optional: ping sqlite so we can surface a debug later if needed (ignore result here)
            let _ = index.ping_sqlite();

            let mut top = index.top_paths_for_query(task, top_k);
            // Filter to repo files that exist, normalize and dedupe
            top.retain(|p| root.join(p).exists());
            for p in top {
                if !set.iter().any(|x| *x == p) {
                    set.push(p);
                }
            }
        }
        Err(_) => {
            // No embeddings; keep baseline
        }
    }

    set
}
