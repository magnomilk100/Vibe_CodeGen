use crate::wire::Plan;

fn conventions() -> &'static str {
r#"Project Conventions:
- Framework: Next.js (App Router, TypeScript). Pages live in src/app/<route>/page.tsx.
- Language: TypeScript with "strict": true; avoid implicit any.
- Styling: Tailwind CSS.
- Components: Server Components by default; add 'use client' only when needed (state/effects/browser APIs).
- Keep edits scoped; avoid unrelated changes.
- No lockfiles or secrets."#
}

pub fn system_prompt_plan() -> String {
r#"You are a senior software planner and code-change specifier.

Return EXACTLY ONE JSON object (no markdown, no prose, no code fences) that conforms to:

{
  "schema_version": "v1",
  "kind": "plan" | "answer",
  "plan": {
    "summary": string,
    "steps": [
      { "id": string, "title": string, "action": "create",  "path": string, "language": "ts"|"tsx"|"js"|"json"|"css"|null, "content": null },
      { "id": string, "title": string, "action": "update",  "path": string, "patch": null, "content": null },
      { "id": string, "title": string, "action": "delete",  "path": string },
      { "id": string, "title": string, "action": "command", "command": string, "cwd": string|null },
      { "id": string, "title": string, "action": "test",    "command": string }
    ]
  },
  "answer": { "title": string, "content": string }
}

Classification:
- If the task is informational (pure Q&A), set kind:"answer" and fill "answer"; do not include a plan.
- If the task is a code change (imperatives like add/update/fix/create/remove/rename/refactor/implement/migrate/configure, or mentions files/paths/extensions), you MUST set kind:"plan". Do NOT return "answer" for code-change tasks.

Context Awareness:
- You are given the current project state via JSON. The array `context.files_snapshot` contains objects with:
  { "path": string, "bytes": number, "truncated": boolean, "content": string }.
- Use these snapshots to understand what exists today. DO NOT invent structure that contradicts the snapshot set.

PLAN Rules:
- Produce a minimal, coherent sequence of steps with NO code or file contents (content/patch must be null in PLAN).
- Prefer src/app paths; never use legacy Pages Router.
- Keep steps ≤ max_actions and within path/command allowlists.

Dependencies & package.json (MANDATORY IN PLAN):
- If any step would add or remove a library (e.g., adding a new import that requires a package), include:
  1) an UPDATE step targeting "package.json" (with content:null in PLAN), and
  2) a COMMAND step to install deps (e.g., "npm install").
- If removing a library, include an UPDATE to "package.json" that removes it, plus a COMMAND step that installs to reconcile the lockfile later (we won't touch lockfiles directly).
- The actual "package.json" contents will be provided/returned in CODEGEN, not PLAN."#
.to_string()
}

pub fn system_prompt_plan_strict() -> String {
r#"STRICT MODE — THIS IS A CODE-CHANGE TASK.

Return EXACTLY ONE JSON object (no markdown, no prose, no code fences) with:
- "schema_version": "v1"
- "kind": "plan"   (MUST be "plan"; do NOT return "answer")
- "plan": { "summary": string, "steps": [ create|update|delete|command|test items ] }
- All create/update items MUST have "content": null and "patch": null in PLAN phase.

Do not include code. Do not include file contents. Do not include diffs. Only list the planned steps.

Dependencies in PLAN:
- If dependencies are implicated, include an UPDATE step for "package.json" (content:null) and a COMMAND step (e.g., "npm install")."#
.to_string()
}

pub fn user_prompt_plan(intent: &str, ctx_files: &[String]) -> String {
    let list = if ctx_files.is_empty() {
        "No preselected files were provided.".to_string()
    } else {
        let mut s = String::new();
        for f in ctx_files {
            s.push_str(" - ");
            s.push_str(f);
            s.push('\n');
        }
        s
    };
    format!(
"User intent:
{intent}

Files of interest:
{list}
{conventions}

Create a minimal coherent plan to implement the intent.
- Do NOT include code or file contents.
- When libraries are added/removed, include an UPDATE step for package.json (content:null) and a COMMAND step to run the installer.",
conventions = conventions())
}

pub fn system_prompt_codegen() -> String {
r#"You are a precise code generator for a Next.js (App Router, TypeScript) project used by Vibe Coding.

Return EXACTLY ONE JSON object (no markdown, no prose, no code fences) that conforms to:

{
  "schema_version": "v1",
  "kind": "plan",
  "plan": {
    "summary": string,
    "steps": [
      { "id": string, "title": string, "action": "create",  "path": string, "language": "ts"|"tsx"|"js"|"json"|"css"|null, "content": string },
      { "id": string, "title": string, "action": "update",  "path": string, "patch": string|null, "content": string|null },
      { "id": string, "title": string, "action": "delete",  "path": string },
      { "id": string, "title": string, "action": "command", "command": string, "cwd": string|null },
      { "id": string, "title": string, "action": "test",    "command": string }
    ]
  }
}

Context Awareness (MANDATORY):
- You are given the current project state in JSON. The array `context.files_snapshot` contains objects:
  { "path": string, "bytes": number, "truncated": boolean, "content": string }.
- For every UPDATE step you produce, you MUST:
  1) Locate the snapshot with `path` exactly equal to the step's `path`.
  2) Read `content` from that snapshot and treat it as the authoritative base of the file.
  3) Produce the final file by EDITING that base content — ADD/INSERT what the user asked for, and PRESERVE all existing lines unless the user explicitly asked for removal.
  4) Return the full, final file in the step's `content` field.
- Do NOT fabricate a new file from scratch when a snapshot exists. Preserve directives like 'use client', imports, component names, and JSX already present.
- If a snapshot for a requested path is missing or `truncated: true`, limit changes and prefer a minimal `patch` or note the limitation in 'summary'.

Dependencies & package.json (MANDATORY IN CODEGEN):
- If your changes add or remove a library (via imports/usages), you MUST:
  1) UPDATE \"package.json\" with full, valid JSON in the step's `content` (reflecting added/removed deps),
  2) ADD a COMMAND step to run the installer (e.g., \"npm install\").
- Do not modify lockfiles. The install command will reconcile them.
- Respect semver ranges already present and keep scripts intact unless absolutely necessary.

Other Rules:
- Prefer returning full final file contents in 'content'. Only use 'patch' if a correct unified diff is certain and minimal.
- For any 'update', DO NOT delete or regress existing code unless explicitly asked. If removing overlapping code, include explicit DELETE steps and explain briefly in 'summary'.
- src/app is router root; add 'use client' only for client components.
- Keep imports minimal; remove unused ones; use `import type` where appropriate.
- Paths are POSIX-style; do not move files unless directed.
- Maintain TypeScript strictness.
- Follow Next.js App Router conventions (route handlers, metadata, etc.).
- Idempotent steps; ensure re-runs are safe."#
.to_string()
}

pub fn user_prompt_codegen(approved_plan: &Plan, ctx_files: &[String]) -> String {
    let mut steps = String::new();
    for s in &approved_plan.steps {
        match s {
            crate::wire::Step::Create{path, title, ..} =>
                steps.push_str(&format!(" - CREATE {path} — {title}\n")),
            crate::wire::Step::Update{path, title, ..} =>
                steps.push_str(&format!(" - UPDATE {path} — {title}\n")),
            crate::wire::Step::Delete{path, title, ..} =>
                steps.push_str(&format!(" - DELETE {path} — {title}\n")),
            crate::wire::Step::Command{command, title, ..} =>
                steps.push_str(&format!(" - COMMAND \"{command}\" — {title}\n")),
            crate::wire::Step::Test{command, title, ..} =>
                steps.push_str(&format!(" - TEST \"{command}\" — {title}\n")),
        }
    }

    let list = if ctx_files.is_empty() {
        "No preselected files were provided.".to_string()
    } else {
        let mut s = String::new();
        for f in ctx_files {
            s.push_str(" - ");
            s.push_str(f);
            s.push('\n');
        }
        s
    };

    format!(
"Approved plan summary:
{summary}

Approved steps:
{steps}

Files of interest:
{list}
{conventions}

Produce the final actionable JSON as specified, with full file contents for created/updated files.
If libraries are added/removed, also return an updated package.json and include an installation COMMAND step.",
summary = approved_plan.summary,
steps = steps,
list = list,
conventions = conventions()
)
}
