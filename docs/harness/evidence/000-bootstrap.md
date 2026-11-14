# Bootstrap evidence — 2026-11-13

Historical snapshot. Build and desktop blockers were subsequently resolved in
[mock-window verification](001-mock-window.md); retain this record as initial evidence.

## Scope and roles

Working branch: `chore/harness-bootstrap`, based on the existing GitHub main.
No implementation commit or upstream goal was created. The source review below
applies to the current working tree, not a committed-SHA gate.

- MAIN performed preflight, prepared and installed templates, ran bootstrap
  checks, recorded findings and coordinated handoffs.
- `harness_planner` inspected the pinned upstream and bounded milestone 001.
- `mock_window_builder` created Cargo.toml, src/main.rs and README.md.
- `bootstrap_reviewer` independently reviewed setup and mock source.

Workers were actually launched with the live collaboration API. The upstream
state machine was not started, and no separate thread-close event was claimed.

## Executed checks

- Pinned archive SHA256 matched `harness/upstream.lock.json`.
- `python3 scripts/install-harness.py <archive> --check`: validated 32 files.
- Approved installation: installed 32 files into project .codex/.agents only.
- Repeat `--check`: installed contents match the prepared files.
- Isolated temporary-directory checks passed for dry-run no-write behavior,
  exact installed contents, model/permission/hook settings, repeat installation,
  refusal to overwrite customized files, and rejection of a bad checksum.
- `bash -n` passed for all eight installed shell scripts and scripts/verify.sh.
- Installed TOML and bootstrap JSON parsed successfully with Python stdlib.
- `bash scripts/verify.sh` exited 2 with `BLOCKED: Cargo is missing`.
  This is an expected missing-tool result, not Rust verification success.
- Upstream `doctor --json`: `ready=false`, `analyze_ready=false`,
  `invalid_roles=[]`; GitHub CLI absent, RTK absent, npx available, and GitNexus
  discoverable. Discoverability does not establish analysis execution.

## Independent review

The reviewer found missing ignore patterns for generated harness runtime data.
Added those patterns, including briefs, drafts, caches, context snapshots and
workflow markers. Follow-up review confirmed they are ignored and templates
remain trackable.

No actionable source defects were found in the mock window by source inspection.
GTK API usage was checked against gtk4 0.11.5 documentation. This does not prove
the source compiles or its widgets work correctly at runtime.

## Still blocked or pending

- Rust/Cargo/rustfmt/Clippy and GTK4 development packages need installation in
  the user's terminal; automated sudo failed because authentication is interactive.
- Cargo.lock generation, formatting, compilation, Clippy and Rust tests: NOT_RUN.
- Application launch, interaction, resizing, closing and Always on Top: NOT_RUN.
- GitHub CLI authentication and RTK availability: pending.
- Native worker lifecycle compatibility, workflow configuration and hook activation:
  pending. Template installation does not establish these capabilities.
- Commit, full analysis, committed-SHA review, PR delivery and merge: NOT_RUN.

## Next action

Install the documented build dependencies, then generate Cargo.lock and execute
scripts/verify.sh. Resolve any findings, run the desktop checklist, and establish
the remaining full-harness prerequisites before invoking the upstream goal skill.
