use std::path::{Component, Path};

/// Ensure `candidate` is within `project_root` or matches an allowlisted file.
pub fn path_is_allowed(candidate: &str, project_root: &str, allowlist: &[String]) -> bool {
    // Direct allow for specific files listed
    if allowlist.iter().any(|p| p.eq_ignore_ascii_case(candidate)) {
        return true;
    }

    // Allow if the first path segment is allowlisted (e.g., "src/**", "app/**", etc.)
    if let Some(first) = Path::new(candidate).components().next() {
        if let Component::Normal(seg) = first {
            let seg = seg.to_string_lossy().to_string();
            if allowlist.iter().any(|allowed| allowed.eq_ignore_ascii_case(&seg)) {
                // also ensure it doesn't escape the root via .. segments
                return is_within_root(candidate, project_root);
            }
        }
    }

    false
}

fn is_within_root(candidate: &str, root: &str) -> bool {
    let abs_root = match std::fs::canonicalize(root) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let joined = Path::new(root).join(candidate);
    match std::fs::canonicalize(joined) {
        Ok(abs_candidate) => abs_candidate.starts_with(&abs_root),
        Err(_) => false,
    }
}

/// Returns true if `cmd` is allowed given the allowlist.
///
/// Rules:
/// - Exact match with an allowlisted command is allowed.
/// - Prefix match is allowed when the command begins with an allowlisted base
///   followed by a single space and arbitrary args, e.g.:
///     allowlist: ["npm install"]  => "npm install next-themes lucide-react" is allowed
/// - Comparison is case-sensitive for safety (shell commands are case-sensitive on *nix).
pub fn command_is_allowed(cmd: &str, allowlist: &[String]) -> bool {
    let trimmed = cmd.trim();

    // Exact match
    if allowlist.iter().any(|base| base == trimmed) {
        return true;
    }

    // Prefix match with args
    for base in allowlist {
        if trimmed.len() > base.len() && trimmed.starts_with(base) {
            // must be base + space + args
            if trimmed.as_bytes()[base.len()] == b' ' {
                return true;
            }
        }
    }

    false
}
