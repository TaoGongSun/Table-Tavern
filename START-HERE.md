# Start Here — Collaborator Onboarding

Welcome! This file is the entry point for a new collaborator working on Table Tavern with an AI assistant (Claude / Fable).

## For you (the human)

Table Tavern is a desktop app (Tauri: React frontend + Rust backend) for multiplayer tabletop roleplay where AI plays the characters and the GM. You bring your own API key or CLI subscription; the app never proxies models.

You don't need to know the codebase — or even pick a task. Open Claude Code in this repository and paste this one sentence:

> Read START-HERE.md and just go.

The assistant will sync, pick the most useful task on its own, do the work, verify it, then commit and push the result together with the handoff files — no questions asked. At the end you get a short report of what was done. You can repeat the same sentence any time (even from your phone); each round is one self-contained, pushed unit of work.

If you ever *do* want to choose or discuss first, say instead: "Read START-HERE.md and show me the task list."

One habit matters: don't close the window until the assistant reports it has **pushed**. Unpushed work is invisible to the other side.

To see the app itself: `npm install` once, then `npm run tauri dev`. The UI has a language switcher (bottom of the left sidebar) — set it to English.

## For the assistant (Claude / Fable)

This collaborator works in **English** — communicate with them in English. Repo documents, code comments, and task files are mostly in Traditional Chinese; translate the relevant parts when relaying them, and keep writing code comments and `.ai/` handoff entries in the repo's existing style.

On "just go" (autonomous mode — the default for this collaborator):

1. `git pull` first. The handoff protocol in `.claude/skills/tavern-handoff/SKILL.md` is the source of truth for the workflow; read `.ai/TASKS.md`, then the chosen task's files under `.ai/tasks/` and `.ai/handoffs/`.
2. Pick the task yourself, in this order: any task in **In progress** first; otherwise the topmost **Todo** task that the assistant can complete without human action. Skip tasks whose next step is human-gated (Chinese markers like 等使用者／使用者操作 — e.g. paid accounts, manual sign-ups, physical-machine testing).
3. Do the work without asking questions. Prefer finishing one small task over half-finishing a big one; if the task is large, complete a coherent slice and leave a precise handoff.
4. Every "done/fixed" claim needs evidence (command output or `file:line`). Verify before pushing: `npm run build`, and `cargo test` in `src-tauri/` when Rust changed.
5. Always finish with the full 收工 flow from the skill (update handoff/task/TASKS.md/history files, commit with the task id, **push**), then report in English: task done, evidence, what's next in the queue. If genuinely blocked, still push the handoff state and say what's blocking in one sentence.

On "show me the task list": summarize in English what's in progress and queued, recommend one, and let them choose before changing anything.

Project ground rules live in `CLAUDE.md` (repo root) and the design docs `NewPlan.md` / `KICKOFF.md`. Verification commands: `npm run build` (frontend typecheck + bundle) and `cargo test` in `src-tauri/`.
