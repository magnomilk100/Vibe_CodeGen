use crate::apply::{ApplyKind, ApplySummary};
use crate::patch::{colorize_preview, Preview};
use crate::wire::{Plan, Step};
use colored::Colorize;
use std::io::{self, Write};

pub fn show_plan(p: &Plan) {
    println!("\n=== PLAN ===");
    println!("{}", p.summary);
    for (i, s) in p.steps.iter().enumerate() {
        match s {
            Step::Create{path, title, ..} =>
                println!("{:>2}. CREATE  {:<40} — {}", i+1, path, title),
            Step::Update{path, title, ..} =>
                println!("{:>2}. UPDATE  {:<40} — {}", i+1, path, title),
            Step::Delete{path, title, ..} =>
                println!("{:>2}. DELETE  {:<40} — {}", i+1, path, title),
            Step::Command{command, title, ..} =>
                println!("{:>2}. COMMAND {:<40} — {} ({})", i+1, command, title, "shell"),
            Step::Test{command, title, ..} =>
                println!("{:>2}. TEST    {:<40} — {}", i+1, command, title),
        }
    }
    println!();
}

pub fn confirm(prompt: &str) -> bool {
    print!("{prompt} [y/N]: ");
    io::stdout().flush().ok();
    let mut s = String::new();
    if io::stdin().read_line(&mut s).is_ok() {
        matches!(s.trim().to_lowercase().as_str(), "y" | "yes")
    } else {
        false
    }
}

pub fn edit_plan(mut plan: Plan) -> Plan {
    println!("Edit mode. Commands:");
    println!("  order <idx...>   e.g., order 2 1 3");
    println!("  drop <idx...>    e.g., drop 3 5");
    println!("  title <idx> <new title...>");
    println!("  show             reprint the plan");
    println!("  done             finish editing\n");

    loop {
        print!("edit> ");
        std::io::stdout().flush().ok();
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).is_err() { break; }
        let line = line.trim();
        if line.is_empty() { continue; }
        let mut parts = line.split_whitespace();
        let cmd = parts.next().unwrap_or("");
        match cmd {
            "order" => {
                let mut new_order: Vec<usize> = Vec::new();
                for p in parts {
                    if let Ok(n) = p.parse::<usize>() {
                        if n>=1 && n<=plan.steps.len() { new_order.push(n-1); }
                    }
                }
                if new_order.len()==plan.steps.len() && uniq(&new_order) {
                    let mut reordered = Vec::with_capacity(plan.steps.len());
                    for idx in new_order { reordered.push(plan.steps[idx].clone()); }
                    plan.steps = reordered;
                    println!("Reordered.");
                } else {
                    println!("Invalid order; must specify each index exactly once.");
                }
            }
            "drop" => {
                let mut to_drop: Vec<usize> = Vec::new();
                for p in parts {
                    if let Ok(n) = p.parse::<usize>() {
                        if n>=1 && n<=plan.steps.len() { to_drop.push(n-1); }
                    }
                }
                to_drop.sort_unstable();
                to_drop.dedup();
                if to_drop.is_empty() {
                    println!("Nothing to drop.");
                } else {
                    let mut kept = Vec::new();
                    for (i, st) in plan.steps.iter().cloned().enumerate() {
                        if !to_drop.contains(&i) { kept.push(st); }
                    }
                    plan.steps = kept;
                    println!("Dropped {} step(s).", to_drop.len());
                }
            }
            "title" => {
                if let Some(idx_str) = parts.next() {
                    if let Ok(mut idx) = idx_str.parse::<usize>() {
                        if idx>=1 && idx<=plan.steps.len() {
                            idx -= 1;
                            let new_title = line.splitn(3, ' ').skip(2).collect::<Vec<_>>().join(" ");
                            if new_title.trim().is_empty() {
                                println!("Provide a new title after the index.");
                            } else {
                                match &mut plan.steps[idx] {
                                    Step::Create{title, ..} |
                                    Step::Update{title, ..} |
                                    Step::Delete{title, ..} |
                                    Step::Command{title, ..} |
                                    Step::Test{title, ..} => *title = new_title.trim().to_string(),
                                }
                                println!("Retitled step {}.", idx+1);
                            }
                        } else { println!("Index out of range."); }
                    }
                }
            }
            "show" => show_plan(&plan),
            "done" => break,
            _ => println!("Unknown command."),
        }
    }
    plan
}

fn uniq(v: &[usize]) -> bool {
    let mut w = v.to_vec();
    w.sort_unstable();
    w.dedup();
    w.len() == v.len()
}

// ===== Preview dashboard =====

pub fn print_preview_dashboard(previews: &[Preview]) {
    let mut creates = 0usize;
    let mut updates = 0usize;
    let mut deletes = 0usize;
    let mut commands = 0usize;
    let mut tests = 0usize;

    for p in previews {
        match p.kind {
            crate::patch::ChangeKind::Create => creates += 1,
            crate::patch::ChangeKind::Update => updates += 1,
            crate::patch::ChangeKind::Delete => deletes += 1,
            crate::patch::ChangeKind::Command => commands += 1,
            crate::patch::ChangeKind::Test => tests += 1,
        }
    }

    println!("\n{}", "┏━━━━━━━━━━━━━━━━━━━━━━━━ Preview ━━━━━━━━━━━━━━━━━━━━━━━━┓".bold());
    println!("  {}: {}   {}: {}   {}: {}   {}: {}   {}: {}",
        "Create".green().bold(), creates,
        "Update".yellow().bold(), updates,
        "Delete".red().bold(), deletes,
        "Command".cyan().bold(), commands,
        "Test".magenta().bold(), tests
    );
    println!("{}", "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛".bold());

    for p in previews {
        println!("{}", colorize_preview(p));
        if p.diff_snippet.is_some() {
            println!();
        }
    }
}

// ===== Final results dashboard =====

pub fn print_apply_dashboard(sum: &ApplySummary) {
    println!("\n{}", "┏━━━━━━━━━━━━━━━━━━━━━━━ Apply Results ━━━━━━━━━━━━━━━━━━━┓".bold());
    println!("  {}: {}   {}: {}   {}: {}   {}: {}   {}: {}   {}: {}   {}: {}B",
        "Created".green().bold(), sum.created,
        "Updated".yellow().bold(), sum.updated,
        "Deleted".red().bold(), sum.deleted,
        "Commands".cyan().bold(), sum.commands_run,
        "Tests".magenta().bold(), sum.tests_run,
        "Skipped".dimmed().bold(), sum.skipped,
        "Bytes".bold(), sum.bytes_written
    );
    println!("{}", "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛".bold());

    for d in &sum.details {
        let label = match d.kind {
            ApplyKind::Created => "[CREATE]".green().bold().to_string(),
            ApplyKind::Updated => "[UPDATE]".yellow().bold().to_string(),
            ApplyKind::Deleted => "[DELETE]".red().bold().to_string(),
            ApplyKind::Command => "[COMMAND]".cyan().bold().to_string(),
            ApplyKind::Test => "[TEST]".magenta().bold().to_string(),
            ApplyKind::Skipped => "[SKIPPED]".dimmed().bold().to_string(),
        };
        let path = d.path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        let before = d.bytes_before.map(|b| format!("{b}B")).unwrap_or_else(|| "-".into());
        let after = d.bytes_after.map(|b| format!("{b}B")).unwrap_or_else(|| "-".into());
        let note = d.note.clone().unwrap_or_default();
        println!("{} {} ({} -> {}) {}", label, path, before, after, note);
    }

    if !sum.command_outputs.is_empty() {
        println!("\n{}", "Cmd/Test Output (truncated)".bold());
        for o in &sum.command_outputs {
            println!("{}", "─".repeat(60).dimmed());
            println!("$ {}", o.command.cyan());
            println!("status: {}  time: {}ms", o.status_code, o.duration_ms);
            let out = o.stdout.trim();
            if !out.is_empty() {
                println!("stdout:\n{}", truncate(out, 800));
            }
            let err = o.stderr.trim();
            if !err.is_empty() {
                println!("stderr:\n{}", truncate(err, 600));
            }
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else {
        format!("{}{}", &s[..max], "\n... (truncated)")
    }
}
