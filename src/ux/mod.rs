use colored::Colorize;
use std::io::{self, Write};

use crate::apply::ApplySummary;
use crate::patch;
use crate::wire::{Plan, Step};

pub fn show_plan(plan: &Plan) {
    println!("\n=== PLAN ===");
    println!("{}", plan.summary.bold());
    if plan.steps.is_empty() {
        println!("(no steps)");
        return;
    }
    for (i, s) in plan.steps.iter().enumerate() {
        match s {
            Step::Create { title, path, .. } => {
                println!("{}. {}  {}", i + 1, "[CREATE]".green().bold(), format!("{} — {}", path, title));
            }
            Step::Update { title, path, .. } => {
                println!("{}. {}  {}", i + 1, "[UPDATE]".yellow().bold(), format!("{} — {}", path, title));
            }
            Step::Delete { title, path, .. } => {
                println!("{}. {}  {}", i + 1, "[DELETE]".red().bold(), format!("{} — {}", path, title));
            }
            Step::Command { title, command, .. } => {
                println!("{}. {}  {}", i + 1, "[COMMAND]".cyan().bold(), format!("{} — {}", command, title));
            }
            Step::Test { title, command, .. } => {
                println!("{}. {}  {}", i + 1, "[TEST]".magenta().bold(), format!("{} — {}", command, title));
            }
        }
    }
    println!();
}

pub fn confirm(prompt: &str) -> bool {
    print!("{} [y/N]: ", prompt);
    let _ = io::stdout().flush();
    let mut s = String::new();
    if io::stdin().read_line(&mut s).is_ok() {
        let ans = s.trim().to_lowercase();
        ans == "y" || ans == "yes"
    } else {
        false
    }
}

/// Minimal inline editor hook. For now, returns the same plan (user may decline and re-run).
/// You can enhance to open $EDITOR or present a TUI later.
pub fn edit_plan(plan: Plan) -> Plan {
    println!("\n(no inline editor configured; returning plan unchanged)\n");
    plan
}

/// Render a compact preview dashboard using patch previews.
/// Counts are inferred from the rendered label (CREATE/UPDATE/DELETE/COMMAND/TEST).
pub fn print_preview_dashboard(previews: &[patch::Preview]) {
    let mut create = 0usize;
    let mut update = 0usize;
    let mut delete = 0usize;
    let mut command = 0usize;
    let mut test = 0usize;

    for p in previews {
        let r = patch::colorize_preview(p);
        if r.contains("[CREATE]") { create += 1; }
        if r.contains("[UPDATE]") { update += 1; }
        if r.contains("[DELETE]") { delete += 1; }
        if r.contains("[COMMAND]") { command += 1; }
        if r.contains("[TEST]") { test += 1; }
    }

    println!(
        "\n{}",
        "┏━━━━━━━━━━━━━━━━━━━━━━━━ Preview ━━━━━━━━━━━━━━━━━━━━━━━━┓".bold()
    );
    println!(
        "  {}: {}   {}: {}   {}: {}   {}: {}   {}: {}",
        "Create".green().bold(), create,
        "Update".yellow().bold(), update,
        "Delete".red().bold(), delete,
        "Command".cyan().bold(), command,
        "Test".magenta().bold(), test
    );
    println!("{}", "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛".bold());

    for p in previews {
        let rendered = patch::colorize_preview(p);
        println!("{}", rendered);
        println!();
    }
}

pub fn print_apply_dashboard(sum: &ApplySummary) {
    println!(
        "\n{}",
        "┏━━━━━━━━━━━━━━━━━━━━━━━ Apply Results ━━━━━━━━━━━━━━━━━━━┓".bold()
    );
    println!(
        "  {}: {}   {}: {}   {}: {}   {}: {}   {}: {}   {}: {}   {}: {}B",
        "Created".green().bold(), sum.created,
        "Updated".yellow().bold(), sum.updated,
        "Deleted".red().bold(), sum.deleted,
        "Commands".cyan().bold(), sum.commands,
        "Tests".magenta().bold(), sum.tests,
        "Skipped".bold(), sum.skipped,
        "Bytes".bold(), sum.bytes
    );
    println!("{}", "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛".bold());

    if !sum.command_outputs.is_empty() {
        println!("{}", "\nCommand outputs:".bold());
        for (i, o) in sum.command_outputs.iter().enumerate() {
            println!(
                "[{}] {}{}",
                i + 1,
                o.command.bold(),
                match &o.cwd {
                    Some(c) => format!("  (cwd: {})", c),
                    None => "".to_string(),
                }
            );
            println!("status: {}  time: {}ms{}", o.status_code, o.duration_ms, if o.via_shell_fallback { "  via-shell" } else { "" });
            if !o.stdout.trim().is_empty() {
                println!("stdout:\n{}", indent(&o.stdout, 2));
            }
            if !o.stderr.trim().is_empty() {
                println!("stderr:\n{}", indent(&o.stderr, 2));
            }
            println!();
        }
    }
}

fn indent(s: &str, n: usize) -> String {
    let pad = " ".repeat(n);
    s.lines()
        .map(|l| format!("{}{}", pad, l))
        .collect::<Vec<_>>()
        .join("\n")
}
