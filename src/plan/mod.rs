use crate::wire::{Plan, Step};
use std::collections::HashMap;

pub fn validate_and_extract(p: Option<&Plan>) -> anyhow::Result<Plan> {
    match p {
        Some(x) => Ok(x.clone()),
        None => Err(anyhow::anyhow!("missing plan")),
    }
}

pub fn coerce(p: Option<&Plan>) -> anyhow::Result<Plan> {
    validate_and_extract(p)
}

/// Sanitize/dedupe plan steps to avoid conflicting/wrong changes.
/// - Deduplicate multiple UPDATEs to the same path (prefer the one with `content`)
/// - Drop UPDATEs that have neither `content` nor `patch`
/// - Keep only one step per (action,path) when applicable
pub fn sanitize(plan: Plan) -> (Plan, Vec<String>) {
    let mut warnings = Vec::new();
    let original_summary = plan.summary.clone();

    // First pass: collect best UPDATE per path
    let mut best_update: HashMap<String, usize> = HashMap::new();
    for (idx, s) in plan.steps.iter().enumerate() {
        if let Step::Update { path, content, patch, .. } = s {
            if content.is_none() && patch.is_none() {
                warnings.push(format!("dropped update for {} (no content or patch)", path));
                continue;
            }
            match best_update.get(path) {
                None => {
                    best_update.insert(path.clone(), idx);
                }
                Some(prev_idx) => {
                    let prev_has_content = matches!(&plan.steps[*prev_idx], Step::Update { content: Some(_), .. });
                    let curr_has_content = content.is_some();
                    if curr_has_content && !prev_has_content {
                        best_update.insert(path.clone(), idx);
                    } else {
                        // keep previous; this will be dropped later
                    }
                }
            }
        }
    }

    // Build new step list preserving order but applying dedupe
    let mut seen_create: HashMap<String, ()> = HashMap::new();
    let mut seen_delete: HashMap<String, ()> = HashMap::new();
    let mut out: Vec<Step> = Vec::new();

    for (idx, s) in plan.steps.into_iter().enumerate() {
        let keep = match &s {
            Step::Update { path, content, patch, .. } => {
                if content.is_none() && patch.is_none() {
                    false
                } else {
                    let keep_idx = best_update.get(path).copied().unwrap_or(idx);
                    keep_idx == idx
                }
            }
            Step::Create { path, .. } => {
                if seen_create.contains_key(path) {
                    warnings.push(format!("dropped duplicate create for {}", path));
                    false
                } else {
                    seen_create.insert(path.clone(), ());
                    true
                }
            }
            Step::Delete { path, .. } => {
                if seen_delete.contains_key(path) {
                    warnings.push(format!("dropped duplicate delete for {}", path));
                    false
                } else {
                    seen_delete.insert(path.clone(), ());
                    true
                }
            }
            _ => true,
        };

        if keep {
            out.push(s);
        } else if matches!(&s, Step::Update { path, .. }) {
            if let Step::Update { path, .. } = &s {
                warnings.push(format!("dropped duplicate update for {}", path));
            }
        }
    }

    (
        Plan {
            summary: original_summary,
            steps: out,
        },
        warnings,
    )
}
