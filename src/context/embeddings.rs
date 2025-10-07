use anyhow::{Context, Result};
use fs_err as fs;
use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct EmbeddingManifest {
    pub chunks: Option<usize>,
    pub collection: Option<String>,
    pub generatedAt: Option<String>,
    pub mirrorPath: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub root: Option<String>,
    pub sqlitePath: Option<String>,
    pub vectorSize: Option<usize>,
    pub version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EmbeddingChunk {
    pub id: String,
    pub path: String,
    pub start: Option<usize>,
    pub end: Option<usize>,
    pub text: String,
    pub lang: Option<String>,
    pub sha1: Option<String>,
}

#[derive(Debug)]
pub struct EmbeddingIndex {
    pub manifest: Option<EmbeddingManifest>,
    pub chunks: Vec<EmbeddingChunk>,
    pub vectors_db: Option<PathBuf>,
}

impl EmbeddingIndex {
    pub fn load(vibe_out: &Path) -> Result<Self> {
        let manifest_path = vibe_out.join("embeddings.manifest.json");
        let jsonl_path = vibe_out.join("embeddings.jsonl");
        let sqlite_path = vibe_out.join("vectors.sqlite");

        let manifest = if manifest_path.exists() {
            let s = fs::read_to_string(&manifest_path)
                .with_context(|| format!("reading {}", manifest_path.display()))?;
            let mf: EmbeddingManifest = serde_json::from_str(&s)
                .with_context(|| format!("parsing {}", manifest_path.display()))?;
            Some(mf)
        } else {
            None
        };

        let mut chunks = Vec::new();
        if jsonl_path.exists() {
            let content = fs::read_to_string(&jsonl_path)
                .with_context(|| format!("reading {}", jsonl_path.display()))?;
            for line in content.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                // Each line should be a JSON object, sometimes nested. Try robust parse:
                if let Ok(val) = serde_json::from_str::<Value>(line) {
                    // The example shows an object-within-object, so dig for fields.
                    // Attempt 1: top-level has the fields directly
                    let (id, path, start, end, text, lang, sha1) = extract_fields(&val)
                        .or_else(|| {
                            // Attempt 2: sometimes the line is the raw JSON object,
                            // but with a nested JSON string under some key; try to decode that
                            if let Some(s) = val.as_str() {
                                serde_json::from_str::<Value>(s)
                                    .ok()
                                    .and_then(|v| extract_fields(&v))
                            } else {
                                None
                            }
                        })
                        .unwrap_or((
                            String::new(),
                            String::new(),
                            None,
                            None,
                            String::new(),
                            None,
                            None,
                        ));
                    if !path.is_empty() && !text.is_empty() {
                        chunks.push(EmbeddingChunk {
                            id,
                            path: normalize_path(&path),
                            start,
                            end,
                            text,
                            lang,
                            sha1,
                        });
                    }
                }
            }
        }

        let vectors_db = if sqlite_path.exists() { Some(sqlite_path) } else { None };

        Ok(Self {
            manifest,
            chunks,
            vectors_db,
        })
    }

    /// Try opening the sqlite to ensure it's readable (optional).
    pub fn ping_sqlite(&self) -> Result<bool> {
        if let Some(p) = &self.vectors_db {
            let _conn = Connection::open_with_flags(
                p,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
            )?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Rank file paths by lexical similarity of chunk text to the query.
    /// Returns unique file paths (normalized, POSIX-ish) ordered by score.
    pub fn top_paths_for_query(&self, query: &str, limit: usize) -> Vec<String> {
        if query.trim().is_empty() || self.chunks.is_empty() {
            return Vec::new();
        }

        let qtokens = tokenize(query);
        if qtokens.is_empty() {
            return Vec::new();
        }

        // Aggregate simple scores per path
        let mut scores: HashMap<String, f32> = HashMap::new();
        for ch in &self.chunks {
            let score = score_text(&ch.text, &qtokens);
            if score > 0.0 {
                *scores.entry(ch.path.clone()).or_insert(0.0) += score;
            }
        }

        let mut pairs: Vec<(String, f32)> = scores.into_iter().collect();
        pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        pairs
            .into_iter()
            .map(|(p, _)| p)
            .take(limit)
            .collect::<Vec<_>>()
    }
}

/// Extract expected fields from a JSON value. The embeddings.jsonl lines can vary,
/// but the example shows keys: id, path, start, end, text, lang, sha1
fn extract_fields(v: &Value) -> Option<(String, String, Option<usize>, Option<usize>, String, Option<String>, Option<String>)> {
    let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let path = v.get("path").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let start = v.get("start").and_then(|x| x.as_u64()).map(|x| x as usize);
    let end = v.get("end").and_then(|x| x.as_u64()).map(|x| x as usize);
    let text = v.get("text").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let lang = v.get("lang").and_then(|x| x.as_str()).map(|s| s.to_string());
    let sha1 = v.get("sha1").and_then(|x| x.as_str()).map(|s| s.to_string());
    Some((id, path, start, end, text, lang, sha1))
}

fn tokenize(s: &str) -> Vec<String> {
    s.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Simple keyword overlap score with log-scaling to reduce spam from very long chunks.
fn score_text(text: &str, qtokens: &[String]) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let ttoks = tokenize(text);
    if ttoks.is_empty() {
        return 0.0;
    }
    let tset: HashMap<&str, usize> = {
        let mut m = HashMap::new();
        for t in &ttoks {
            *m.entry(t.as_str()).or_insert(0) += 1;
        }
        m
    };
    let mut hits = 0usize;
    for q in qtokens {
        if tset.contains_key(q.as_str()) {
            hits += 1;
        }
    }
    if hits == 0 {
        return 0.0;
    }
    let len_penalty = (ttoks.len() as f32).ln().max(1.0);
    (hits as f32) / len_penalty
}

/// Normalize backslashes into forward slashes for consistency.
fn normalize_path(p: &str) -> String {
    p.replace('\\', "/")
}
