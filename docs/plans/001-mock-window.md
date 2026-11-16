# Milestone 001: mock lyrics window

## Objective

Build the smallest useful Rust + GTK4 window to validate desktop behavior on
Ubuntu 26.04 GNOME Wayland before connecting Spotify or fetching lyrics.

## Scope

- A single resizable GTK4 application window with a normal title bar.
- Display original sample text, clearly identified as a demonstration.
- Show a current line and surrounding context; advance with a button.
- Close normally through the window manager.
- Document manual pinning through GNOME's window menu.

No network access, Spotify integration, timers, timestamp parsing, preferences,
system tray, automatic pinning, transparency, or packaging in this milestone.

## Task ownership

1. Planner: confirm bounded scope and observable acceptance criteria (read-only).
2. Builder: `Cargo.toml`, `Cargo.lock`, `src/**`, and `README.md` only.
3. MAIN: run verification, collect evidence, and reconcile handoff.
4. Reviewer: inspect implementation and check results independently (read-only).
5. Desktop acceptance: launch on the actual laptop and observe pinning/resizing.

## Automated acceptance

Run `bash scripts/verify.sh`. It requires formatting, locked build, Clippy with
warnings denied, and tests to succeed. Do not introduce tests that only mirror
trivial GTK widget construction. Zero tests is reported as zero tests, not UI
coverage. Generate and retain Cargo.lock when Cargo becomes available.

## Desktop acceptance

Run `cargo run --locked`, then record observations for:

- Initial text is readable; current and surrounding lines are distinguishable.
- Advancing lines updates the displayed context and handles the end of the sample.
- Resize to a narrow window; long lines remain readable through wrapping.
- Use the title-bar window menu (or Alt+Space) and select Always on Top if offered.
- Switch to an editor/browser and confirm whether the lyric window remains above it.
- Close normally and confirm the process exits.

Record the GNOME/Wayland session and limitations. Screenshots alone do not prove
pinning. Full-screen apps, other workspaces, transparency and click-through are
outside this acceptance contract.

## Learning outcome

Trace requirement → brief → implementation → check results → independent review
→ desktop observations. Identify which evidence is missing before deciding the
milestone is complete. This Markdown is task input, not proof of execution.
