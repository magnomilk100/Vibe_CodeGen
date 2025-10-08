use clap::Parser;
use uuid::Uuid;
use chrono::Utc;
use serde_json::json;
use std::path::Path;

mod cli;
mod config;
mod provider;
mod context;
mod wire;
mod plan;
mod patch;
mod apply;
mod safety;
mod exec;
mod git;
mod log;
mod errors;
mod prompt;
mod ux;
mod merge;

fn is_code_action(task: &str) -> bool {
    let t = task.to_lowercase();
    let verbs = [
        "add", "update", "fix", "create", "delete", "remove", "rename",
        "refactor", "implement", "migrate", "configure", "change", "patch",
        "insert", "modify",
    ];
    if verbs.iter().any(|v| t.contains(v)) {
        return true;
    }
    let file_hints = [".ts", ".tsx", ".js", ".json", ".css", "src/app", "page.tsx", "layout.tsx"];
    file_hints.iter().any(|h| t.contains(h))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = cli::Args::parse();

    let mut cfg = config::Config::default();
    cfg.root = args.root.clone();

    let txid = Uuid::new_v4();
    if args.debug {
        println!("debug: flag enabled");
        log::print_planned_paths(Path::new(&cfg.root), txid);
    }

    let root = Path::new(&cfg.root);
    let vibe_out = Path::new(&args.vibe_out);

    // embeddings-aware selection + baseline (always includes package.json)
    let ctx_files = context::select_relevant_files(
        args.task.as_deref().unwrap_or(""),
        root,
        vibe_out,
        12,
    );

    let prov = provider::make_provider(
        args.provider.clone(),
        args.model.clone(),
        args.timeout_secs,
        cfg.ollama_url.clone(),
    )?;

    // ===== PHASE 1: PLAN =====
    let plan_files_snapshot = context::snapshot_files(&ctx_files, root, 8_192);
    let mut plan_req = wire::LlmRequest {
        schema_version: "v1".into(),
        mode: wire::Mode::Plan,
        transaction: wire::Tx { id: txid, timestamp: Utc::now(), dry_run: args.dry_run },
        limits: wire::Limits {
            max_actions: cfg.max_actions,
            max_patch_bytes: cfg.max_patch_bytes,
            allowed_commands: cfg.command_allowlist.clone(),
        },
        task: args.task.clone().unwrap_or_default(),
        context: wire::ContextSlice {
            summary: json!({ "router":"App", "typescript": true, "note": "PLAN phase request" }),
            files_index: vec![],
            routes: vec![],
            symbols: json!({}),
            diagnostics: vec![],
            files_snapshot: plan_files_snapshot,
        },
        capabilities: vec!["fs.apply_patch".into(),"tests.run".into(),"cmd.run".into()],
        safety: wire::Safety { path_allowlist: cfg.path_allowlist.clone(), command_allowlist: cfg.command_allowlist.clone() },
        instruction: wire::Instruction {
            system: prompt::system_prompt_plan(),
            user: prompt::user_prompt_plan(args.task.as_deref().unwrap_or(""), &ctx_files),
            developer: Some("Output exactly one JSON object; PLAN must not include file contents. If libraries are added/removed, include UPDATE package.json (content:null) and a COMMAND step to run installer.".to_string()),
        },
    };

    let mut plan_resp = prov.send(&plan_req, args.debug).await?;
    let saved_plan = log::save_stage("plan", &plan_req, &plan_resp, txid, &cfg, args.save_request, args.save_response)?;
    if args.debug {
        log::print_saved_paths("plan", &saved_plan);
        log::print_json_debug("plan", &plan_req, &plan_resp)?;
    }

    let is_code = is_code_action(args.task.as_deref().unwrap_or(""));
    let answer_present = plan_resp.answer.is_some();
    let need_strict = (matches!(plan_resp.kind, wire::Kind::Answer)
        || plan_resp.plan.as_ref().map(|p| p.steps.is_empty()).unwrap_or(true)
        || (answer_present && is_code));

    if need_strict {
        let mut strict_req = plan_req.clone();
        strict_req.instruction.system = prompt::system_prompt_plan_strict();
        strict_req.instruction.developer = Some("STRICT MODE: This is a code-change task. Return kind:\"plan\" ONLY. Do not include code, content or patches in PLAN. Do not include an 'answer' field. If dependencies are implicated, include UPDATE package.json (content:null) and a COMMAND step to run installer.".to_string());
        let strict_resp = prov.send(&strict_req, args.debug).await?;
        let saved_plan_strict = log::save_stage("plan.strict", &strict_req, &strict_resp, txid, &cfg, args.save_request, args.save_response)?;
        if args.debug {
            log::print_saved_paths("plan.strict", &saved_plan_strict);
            log::print_json_debug("plan.strict", &strict_req, &strict_resp)?;
        }
        plan_req = strict_req;
        plan_resp = strict_resp;
    }

    if matches!(plan_resp.kind, wire::Kind::Answer) {
        if let Some(ans) = plan_resp.answer {
            println!("\n=== ANSWER ===\n{}\n\n{}\n", ans.title, ans.content);
        } else {
            println!("\n=== ANSWER ===\n(model returned no answer payload)\n");
        }
        return Ok(());
    }

    let mut approved_plan = match plan_resp.plan {
        Some(p) if !p.steps.is_empty() => p,
        _ => {
            println!("Model did not return a usable plan.");
            return Ok(());
        }
    };

    // Show plan & ask for confirmation (user may edit once)
    ux::show_plan(&approved_plan);
    let mut proceed = ux::confirm("Apply this plan? (enter 'n' to edit)");
    if !proceed {
        approved_plan = ux::edit_plan(approved_plan);
        ux::show_plan(&approved_plan);
        proceed = ux::confirm("Apply this edited plan?");
    }
    if !proceed {
        println!("Aborted by user.");
        return Ok(());
    }

    // ===== PHASE 2: CODEGEN =====
    let codegen_files_snapshot = context::snapshot_files(&ctx_files, root, 300_000);

    // NEW: pass original task + prior PLAN prompts to CODEGEN user prompt (for rich continuity)
    let codegen_user = prompt::user_prompt_codegen(
        args.task.as_deref().unwrap_or(""),
        &approved_plan,
        &ctx_files,
        &plan_req.instruction.system,
        &plan_req.instruction.user,
        plan_req.instruction.developer.as_deref(),
    );

    let codegen_req = wire::LlmRequest {
        schema_version: "v1".into(),
        mode: wire::Mode::Codegen,
        transaction: wire::Tx { id: txid, timestamp: Utc::now(), dry_run: args.dry_run },
        limits: wire::Limits {
            max_actions: cfg.max_actions,
            max_patch_bytes: cfg.max_patch_bytes,
            allowed_commands: cfg.command_allowlist.clone(),
        },
        task: args.task.clone().unwrap_or_default(),
        context: wire::ContextSlice {
            summary: json!({ "router":"App", "typescript": true, "note": "CODEGEN phase request" }),
            files_index: vec![],
            routes: vec![],
            symbols: json!({}),
            diagnostics: vec![],
            files_snapshot: codegen_files_snapshot,
        },
        capabilities: vec!["fs.apply_patch".into(),"tests.run".into(),"cmd.run".into()],
        safety: wire::Safety { path_allowlist: cfg.path_allowlist.clone(), command_allowlist: cfg.command_allowlist.clone() },
        instruction: wire::Instruction {
            system: prompt::system_prompt_codegen(),
            user: codegen_user,
            developer: Some("Return full file contents in 'content' for created/updated files; prefer 'content' over 'patch'. Never remove top-of-file directives like 'use client' unless explicitly asked. If libraries are added/removed, also UPDATE package.json (full JSON) and add a COMMAND step to run 'npm install'. Use context.files_snapshot as the source of truth for existing files.".to_string()),
        },
    };

    let codegen_resp = prov.send(&codegen_req, args.debug).await?;
    let saved_codegen = log::save_stage("codegen", &codegen_req, &codegen_resp, txid, &cfg, args.save_request, args.save_response)?;
    if args.debug {
        log::print_saved_paths("codegen", &saved_codegen);
        log::print_json_debug("codegen", &codegen_req, &codegen_resp)?;
    }

    let raw_plan = match codegen_resp.plan {
        Some(p) => p,
        None => { println!("\n(no code changes returned by model)\n"); return Ok(()); }
    };

    let (plan_filtered, warnings) = plan::sanitize(raw_plan);
    if !warnings.is_empty() {
        println!("\nSanitizer warnings:");
        for w in warnings { println!(" - {}", w); }
    }

    safety::validate(&plan_filtered, &cfg)?;
    let previews = patch::preview(root, &plan_filtered, args.task.as_deref().unwrap_or(""))?;
    ux::print_preview_dashboard(&previews);

    if !ux::confirm("Proceed to apply these changes?") {
        println!("Aborted by user.");
        return Ok(());
    }

    let summary = apply::apply_steps(
        root,
        &plan_filtered.steps,
        args.dry_run,
        &cfg,
        args.task.as_deref().unwrap_or(""),
    )?;
    ux::print_apply_dashboard(&summary);

    Ok(())
}
