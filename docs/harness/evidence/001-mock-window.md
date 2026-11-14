# Mock window verification — 2026-11-13

## Outcome

Milestone 001 is functionally verified as a delegated local pilot. It is not a
completed upstream `$goal` run, committed-SHA review, or PR delivery. Application
source is unchanged from the independent source review; Cargo.lock was generated
and reviewed in this follow-up. All changes remain in the working tree.

## Installed build tools

- rustc and Cargo 1.93.1.
- Clippy 0.1.93 and rustfmt available.
- GTK4 development metadata 4.22.4.
- Cargo.lock generated with 64 Rust 1.93-compatible registry dependencies.

Cargo uses `/tmp/spotify-floating-lyric-cargo` in this agent session to avoid
changing the user's global Cargo configuration or cache. Initial sandbox DNS
failed; lock generation and verification were rerun with approved network access.

## Automated verification

`env CARGO_HOME=/tmp/spotify-floating-lyric-cargo bash scripts/verify.sh` exited 0:

- `cargo fmt --all -- --check`: PASS.
- `cargo build --locked`: PASS.
- `cargo clippy --locked --all-targets -- -D warnings`: PASS.
- `cargo test --locked`: PASS, **zero tests**. This is not automated UI coverage.

The independent Reviewer checked the lockfile: gtk4 0.11.5 matches the manifest;
all registry packages have checksums and crates.io sources. No new concrete
findings. Review is of the working tree, not an upstream committed-SHA gate.

## Desktop acceptance

MAIN launched `./target/debug/spotify-floating-lyric` on the actual laptop using
approved desktop access. The user tested Previous/Next, resizing, manual Always
on Top while switching applications, and closing, then explicitly reported:
"Controls, resizing, pinning, and closing all worked."

The process subsequently exited 0. These are user-observed desktop results plus
a process exit check, not agent visual inspection or screenshot evidence.
Fullscreen behavior, other workspaces, transparency and click-through remain
outside the milestone contract.

## Harness tools and remaining setup

GitHub CLI 2.46.0 is installed and its needed flags are supported. The latest
approved `doctor --json` check still reports GitHub authentication false.

Installed official RTK OSS 1.2.0 under ignored `.tools/bin`, with its release
archive SHA256 verified against GitHub release metadata. Provenance is recorded
in `harness/tools.lock.json`; the license is retained beside the local tools.
`rtk --version` and `rtk gain` succeeded. Gain reported no tracking data, not
measured token savings. Those probes used temporary XDG data/config directories.

With `.tools/bin` on PATH, doctor reports RTK/npx/GitNexus discoverable,
`invalid_roles=[]`, `analyze_ready=true`, and `ready=false` because GitHub is not
authenticated. This is capability discovery; no mandatory analysis was executed.
Native lifecycle validation, workflow configuration, hook activation and actual
PR delivery remain outstanding. No global trust, authentication, or merge was
performed by the agent.

## Next action

The user signs in with `gh auth login --hostname github.com`. Then configure and
validate the full workflow executor before using it for Spotify/MPRIS work.
Do not mark a full harness goal complete from this local pilot's evidence.
