use std::cmp::max;

pub fn is_additive_task(task: &str) -> bool {
    let t = task.to_lowercase();
    let add_kw = ["add", "append", "insert", "another", "extra", "include", "augment"];
    let destructive_kw = ["remove", "delete", "replace", "overwrite", "rewrite", "refactor", "rework"];
    add_kw.iter().any(|k| t.contains(k)) && !destructive_kw.iter().any(|k| t.contains(k))
}

pub fn has_use_client_top(src: &str) -> bool {
    for line in src.lines().take(10) {
        let l = line.trim_start_matches('\u{feff}').trim();
        if l.is_empty() { continue; }
        if l.starts_with("//") { continue; }
        if l.starts_with("/*") { continue; }
        if l.starts_with("import ") { return false; }
        if l == "'use client'"
            || l == "\"use client\""
            || l == "'use client';"
            || l == "\"use client\";"
        {
            return true;
        }
        return false;
    }
    false
}

pub fn preserve_use_client(old: Option<&str>, new_content: &str, task: &str) -> String {
    let wants_removal = {
        let t = task.to_lowercase();
        t.contains("remove 'use client'") || t.contains("remove use client")
    };
    if wants_removal { return new_content.to_string(); }
    if let Some(old_src) = old {
        if has_use_client_top(old_src) && !has_use_client_top(new_content) {
            let mut s = String::from("'use client'\n\n");
            s.push_str(new_content.trim_start_matches('\u{feff}'));
            return s;
        }
    }
    new_content.to_string()
}

/// Line-based LCS to build an additive merge:
/// - Keep all original lines
/// - Insert new lines where they don't match (ignore deletions)
pub fn additive_merge(old: &str, new_content: &str) -> String {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new_content.lines().collect();
    let n = a.len();
    let m = b.len();

    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if a[i] == b[j] {
                1 + dp[i + 1][j + 1]
            } else {
                max(dp[i + 1][j], dp[i][j + 1])
            };
        }
    }

    let mut i = 0usize;
    let mut j = 0usize;
    let mut out: Vec<String> = Vec::with_capacity(n + m);

    while i < n && j < m {
        if a[i] == b[j] {
            out.push(a[i].to_string());
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            // deletion in model proposal => keep original
            out.push(a[i].to_string());
            i += 1;
        } else {
            // insertion or replacement => add model's line without removing original
            out.push(b[j].to_string());
            j += 1;
        }
    }

    while i < n {
        out.push(a[i].to_string());
        i += 1;
    }
    while j < m {
        out.push(b[j].to_string());
        j += 1;
    }

    // Cleanup: collapse consecutive identical lines to avoid duplicates
    let mut cleaned: Vec<String> = Vec::with_capacity(out.len());
    for line in out {
        if cleaned.last().map(|s| s == &line).unwrap_or(false) {
            continue;
        }
        cleaned.push(line);
    }
    cleaned.join("\n")
}
