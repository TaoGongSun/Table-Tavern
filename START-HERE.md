# Start Here — Collaborator Onboarding

Welcome! This file is the entry point for a new collaborator working on Table Tavern with an AI assistant (Claude / Fable).

## For you (the human)

Table Tavern is a desktop app (Tauri: React frontend + Rust backend) for multiplayer tabletop roleplay where AI plays the characters and the GM. You bring your own API key or CLI subscription; the app never proxies models.

You don't need to know the codebase to get started. Open Claude Code in this repository and paste:

> Read START-HERE.md and pick up a task.

That's it. The assistant will pull the latest state, show you what's in progress and what's queued, and ask which task you want. When a work session ends, the assistant updates the handoff files and pushes, so the other collaborator (and their assistant) can pick up exactly where you left off.

Two habits that keep the collaboration smooth:

1. **Start of session**: always begin with the sentence above (or "continue the current task") so the assistant syncs before touching anything.
2. **End of session**: say "wrap up" / "收工" and let the assistant finish the handoff flow (update `.ai/` files, commit, push) before you close the window. Unpushed work is invisible to the other side.

To see the app itself: `npm install` once, then `npm run tauri dev`. The UI has a language switcher (bottom of the left sidebar) — set it to English.

## For the assistant (Claude / Fable)

This collaborator works in **English** — communicate with them in English. Repo documents, code comments, and task files are mostly in Traditional Chinese; translate the relevant parts when relaying them, and keep writing code comments and `.ai/` handoff entries in the repo's existing style.

On "pick up a task" (or any session start):

1. `git pull` first.
2. Follow the project handoff protocol in `.claude/skills/tavern-handoff/SKILL.md` (source of truth for the workflow). In short: read `.ai/TASKS.md`, then the chosen task's `.ai/tasks/<task-id>.md` and, if present, `.ai/handoffs/<task-id>.md`.
3. Summarize for the collaborator in English: what's in progress, what's queued, and a recommended next task. Let them choose before making changes.
4. Work on one task at a time; every "done/fixed" claim needs evidence (command output or `file:line`).
5. On wrap-up, run the full 收工 flow from the skill (update handoff/task/TASKS.md/history files, commit with the task id, push).

Project ground rules live in `CLAUDE.md` (repo root) and the design docs `NewPlan.md` / `KICKOFF.md`. Verification commands: `npm run build` (frontend typecheck + bundle) and `cargo test` in `src-tauri/`.
