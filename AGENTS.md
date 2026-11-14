# Spotify Floating Lyric

Build an Ubuntu GNOME app with Rust and GTK4. Begin with the mock window in
`docs/plans/001-mock-window.md`; Spotify integration and lyric fetching follow.

## Working agreement

- MAIN coordinates requirements, task briefs, verification, review, and delivery.
  Delegate application edits to one Builder at a time. Use separate read-only
  Planner and Reviewer workers. Workers never delegate or publish.
- Read `docs/harness/bootstrap.md` before starting the upstream goal workflow.
  Bootstrap is not an active or completed upstream goal. Never fabricate gate,
  worker-closure, visual, or PR evidence to make a workflow advance.
- Keep changes within this repository. Preserve the existing MIT license and
  unrelated user changes. No automatic merge or global configuration edits.
- Use per-change branches and PRs targeting `development`, merged with squash.
  Release PRs must run from `development` to `main` and use a normal merge
  without squashing. Do not merge directly from a feature branch to `main`.
- Preserve the session's permission controls. Missing tools are blockers, not
  application defects. Never mark NOT_RUN or UNKNOWN checks as PASS.
- Use `scripts/verify.sh` for Rust verification. Commit Cargo.lock for this app.
  Formatting/build/lint tests do not prove desktop behavior.
- Prepend the workflow root's `.tools/bin` to PATH for helper commands; RTK is
  installed there. Run `python3 scripts/test-harness.py` for harness QA; it resolves
  the shared installation and verified archive cache even from a task worktree.
- Keep normal window decorations for the initial Wayland milestone. Do not
  promise automatic always-on-top, click-through, or cross-workspace behavior.
- Show original mock text; do not add real song lyrics to fixtures.
- Explain the task boundary, checks, review findings, and next action as we work.

## Native delegation in this session

Use the live `collaboration.spawn_agent` schema, with `task_name` and `message`.
Embed role and file ownership in each brief; named agent TOMLs are not loaded
automatically by that API. There are four total slots including MAIN. Start
sequentially. Do not pass the upstream example's unsupported `agent_type` or
`fork_context` fields or claim a separate close action occurred.

- Start every assignment with `fork_turns: "none"` and a fresh worker using the
  unique reserved task name. Pass the reserved model and effort explicitly.
- Keep briefs focused on the objective, relevant files, constraints, acceptance
  checks, required handoff, and required role/assignment metadata. Omit chat history;
  workers may read additional relevant source as needed.
- Never resume or reuse a finished, released worker. Fixes go to a new worker
  with the findings and relevant current context.
- Workers share the filesystem. Assign file ownership and use separate worktrees
  for concurrent implementation.

The pinned upstream skill is a reference for the full workflow. Its lifecycle
must pass capability checks in the executor actually running it before `$goal`
is invoked. See `docs/harness/bootstrap.md` for outstanding prerequisites.
Use the installed lifecycle reference for the configured observation mode;
record real terminal observations and never resume a released worker.
