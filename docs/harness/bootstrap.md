# Harness bootstrap and learning log

## Upstream and adoption boundary

Reference: https://github.com/vinutama/ai-prerequisites

Pinned commit: `c2d30fff4a12c5d646536c3f511f6e18e2482f73`.

The existing local reference checkout was older, so setup uses the inspected
pinned archive. Do not execute upstream `init.sh` blindly: it can write global
project trust and its Codex template requests unrestricted execution.

This bootstrap prepares project-scoped templates without invoking `$goal`,
starting a harness state machine, editing global trust, installing optional
skills, publishing, or merging. A delegated local bootstrap is not a completed
upstream harness run.

## Reproducing template installation

Download the pinned archive:

```sh
curl -fL https://api.github.com/repos/vinutama/ai-prerequisites/tarball/c2d30fff4a12c5d646536c3f511f6e18e2482f73 -o /tmp/ai-prerequisites.tar.gz
python3 scripts/install-harness.py /tmp/ai-prerequisites.tar.gz
```

The installer checks the archive SHA256 against `harness/upstream.lock.json`,
prepares only Codex templates, refuses different existing destination files,
and reports installation counts. A changed GitHub archive must be investigated
and the lock deliberately refreshed rather than skipping checksum verification.

The installed `.codex` and `.agents` trees are ignored runtime installations,
matching the upstream scaffold's storage model. The tracked installer, upstream
lock, and reviewed adapter patches reproduce them. Keep progress, observations,
local configuration and tool caches out of application commits.

The original base adaptation omitted main-session permission overrides,
restricted Builder sandboxes to workspace writes and other role sandboxes to
read-only, disabled hooks pending explicit activation, capped the total thread
count at four, and used the available `gpt-6.1-sol` routing. The subsequently
approved lifecycle and branch-flow adapter is a versioned patch applied after
archive verification. Its inputs, outputs and patch bytes are checksum-bound in
the upstream lock. All adapter development and installation happens before a
real goal run.

Existing exact normalized-base installations can be upgraded with
`python3 scripts/install-harness.py <archive> --upgrade-reviewed`; arbitrary
customizations are refused. A normal reinstall also refuses unexpected changes.

The upstream instructions still describe native Codex role dispatch. This chat
uses `collaboration.spawn_agent`, which does not load role TOMLs implicitly or
expose a separate agent-close operation. Do not assert `--closed` or treat a
launch reservation as an actual launched worker. The full lifecycle must be
validated in the chosen executor before starting a goal. The opt-in adapter
uses saved, validated terminal observations with `closed=false` and leaves
native close-based mode as the default. See the installed goal skill's
`references/lifecycle.md` after upgrading.

## Observed preflight, 2026-11-13

- Ubuntu 26.04.1 LTS, GNOME, Wayland.
- Codex CLI 0.167.2; Git, jq, Python, Node and pkg-config available.
- `org.mpris.MediaPlayer2.spotify` present on the session bus. This establishes
  discoverability only; metadata, position accuracy and notifications are untested.
- Rust, Cargo, rustfmt, Clippy and GTK4 development metadata initially absent.
- System installation needs interactive sudo authentication in the user's terminal.
- Parent Git discovery initially resolved to a repository containing sibling
  projects. Created a separate repository here, fetched the user-supplied
  `https://github.com/samm002/spotify-floating-lyric.git`, and branched from its
  existing main to `chore/harness-bootstrap`, preserving LICENSE.
- GitHub CLI and RTK initially absent. No credentials or global config inspected.

## Before the first full upstream goal

1. Install build dependencies in the user's terminal:
   `sudo apt-get install rustc cargo rustfmt rust-clippy libgtk-4-dev`.
2. Install/configure GitHub CLI for this repository and RTK. See the local RTK
   setup below. The upstream mandatory analysis command runs
   `npx --yes gitnexus@1.6.9 analyze` and `rtk gain`.
3. Confirm the executor supports actual worker dispatch and completion lifecycle;
   do not infer support from a version string. Validate writable runtime paths.
4. Apply the user-confirmed project policy through the helper, using
   `python3 scripts/configure-harness.py --check` then
   `python3 scripts/configure-harness.py`. This setup-only script refuses existing
   goal state. The tracked policy selects Markdown plus explicit GitHub issue
   runs, target development, one worker, inline PR review, manual merging, and
   conditional visual acceptance. A source `.md` remains useful.
5. Set explicit `verify_commands` from `harness/verify-commands.json`. Missing
   Cargo must not silently remove checks through auto-detection.
6. Approve/enable hooks through the supported client flow if desired. Prove hooks
   execute; an installed hooks.json is not evidence of execution.
7. Snapshot the full milestone source before branch/worktree mutations. Only then
   start the full goal and record real worker, commit, analysis, verify, review,
   desktop, and PR delivery evidence.

Do not use `state complete` without validated PR delivery. `review_mode=local`
delays publication; it is not local-only completion. Stop with a saved blocker
when a required capability is unavailable; do not rewrite helpers mid-run.

## Learning sequence

- Bootstrap: understand roles, capability checks and evidence boundaries.
- Mock window: complete one sequential implementation/review cycle.
- Playback: add MPRIS discovery, metadata, pause/seek/track changes and reconnect.
- Lyrics: add provider matching, local cache, and timestamp parsing with fixtures.
- Synchronization: test a monotonic clock against pause, seek and track changes.
- Recovery/parallelism: resume an interrupted run; delegate independent work only
  after interfaces and file ownership are established.

Each handoff states files, commands/results, blockers, and the next action.
Automated checks do not establish GNOME pinning behavior or live Spotify accuracy.

## Current handoff

See [bootstrap evidence](evidence/000-bootstrap.md) for the initial setup and
[mock-window verification](evidence/001-mock-window.md) for the follow-up.
Milestone 001 passes automated checks and user-observed desktop acceptance as a
local pilot. [Adapter validation](evidence/003-adapter-validation.md) records the
installed and tested lifecycle adapter and saved project policy. A full upstream
application goal has not started. Only the development base branch was created;
no implementation source was pushed and no PR or merge was performed.

[Setup finalization](evidence/004-setup-finalization.md) records the 2026-11-14
model-policy reconciliation, enabled and trusted project hooks, live hook smoke,
and current verification. The older evidence files remain historical snapshots.

Keep a clean bootstrap checkpoint before the first real goal creates task
worktrees. Scoped ordinary Git commands may seed that
checkpoint outside the goal state machine; this is bootstrap maintenance, not
a bypass of an active goal's gates. Once a goal starts, its Git/forge mutations
use the operational helper. A bootstrap checkpoint does not assert full goal
completion or authorize publication/merging.

## Project-local RTK

RTK OSS 1.2.0 is installed in ignored `.tools/bin` with provenance recorded in
`harness/tools.lock.json`. Its official installation guide warns that the plain
`cargo install rtk` name may refer to an unrelated project, so use the recorded
official binary or explicit official Git URL:
https://github.com/rtk-ai/rtk/blob/master/docs/guide/getting-started/installation.md

To reproduce the local binary, download the archive in tools.lock.json, verify
its exact SHA256 before extraction, then extract only `rtk` into `.tools/bin`
and retain its LICENSE in `.tools/licenses`. Do not execute an unverified archive.

Run the helper with the project tools visible:

```sh
PATH="$PWD/.tools/bin:$PATH" .codex/scripts/goal-git.sh doctor --json
```

The first verification snapshot reported GitHub not signed in. The user
subsequently signed in successfully; [authentication evidence](evidence/002-auth-and-lifecycle.md)
records that result and the separate lifecycle gap. The adapter and latest hook
validation are recorded in the subsequent evidence entries. Dependency
readiness is separate from a working execution lifecycle.

## Branch and source policy

`harness/project-policy.json` records the user's choices independently of ignored
runtime configuration. Markdown files are the default; explicit issue runs use
`https://github.com/samm002/spotify-floating-lyric/issues`. The agent chooses
manageable batches (initially three issues), continuing as needed for the goal,
with one active worker initially. Issue batches are not a project-scope limit.

Use per-change branches and PRs into `development`, reviewed inline and merged
with squash. Release PRs go from `development` to `main` using a normal merge
without squashing. No feature branch goes directly to main. Automatic merge is
off. The remote development branch was established from the existing main commit
before publishing any implementation. The opt-in `branch_flow` helper policy
selects the correct merge method if a merge is explicitly requested later.

## Model and effort policy

The source lock reproduces the chosen routing: MAIN defaults to `gpt-6.1-sol`
with high effort; Builder uses `gpt-6-luna` with `xhigh`; Researcher uses
`gpt-6.1-sol` with low effort at every complexity level; and Visual Reviewer
uses `gpt-6.1-sol` with low effort at every complexity level. Other role routes
are unchanged. Use explicit routed model/effort and fresh briefs when launching
workers. Higher effort can consume more reasoning tokens; model selection alone
does not guarantee account quota savings.

Project configuration supplies defaults for clients that load it. It does not
switch an already running chat: select Sol 6.1 High in the active model picker
when using this chat as MAIN. Project installation does not write global
configuration; client hook trust is handled separately below.

## Desired project hooks and persistent source

As of 2026-11-14, `harness/upstream.lock.json` is the persistent source for
model routing and the desired project hook setting. Editing ignored `.codex`
runtime files alone does not survive a reviewed reinstall. The installer maps
`hooks_enabled` into `.codex/config.toml` and roots each goal helper command at
the directory containing Git's shared common directory. This supports
subdirectories and linked task worktrees whose top-level directory has no
`.codex` copy.

The lock and installed runtime now both enable hooks. Client trust is managed
separately from the project lock, and the installer never writes approval records.
The user approved a narrow exception to the no-global-configuration rule for
these three repository-specific hook approvals through Codex's supported
`/hooks` flow. Unrelated user settings and other hook approvals were checked
and remained unchanged. This is not blanket authorization for future global
changes or bypassing hook review.

The live smoke verified helper execution for all three hooks without starting
an application goal. Script tests separately cover event recording, continuation,
saved blockers, no-state behavior, and the repeated-stop limit. See
[setup finalization](evidence/004-setup-finalization.md) for evidence and limits.
Fresh clients must still discover and trust the exact hook definitions through
their supported review flow; changed definitions may need another review.
