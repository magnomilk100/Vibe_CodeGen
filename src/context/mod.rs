use crate::wire::FileBlob;
use fs_err as fs;
use std::path::Path;

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
                hash: None, // can be filled later if we decide to hash
                truncated,
                content,
            }),
            Err(_) => {
                // Best-effort: skip unreadable files without failing the whole request
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
