# Authentication and lifecycle preflight — 2026-11-13

## Verified access

After the user completed GitHub CLI login, the approved project helper's
`doctor --json` reported GitHub authenticated, expected CLI flags available,
`invalid_roles=[]`, `analyze_ready=true`, and `ready=true` with `.tools/bin` on
PATH. This proves dependency/authentication discovery, not goal execution.

The helper's `issues list https://github.com/samm002/spotify-floating-lyric/issues 1`
returned `[]` with exit 0: access works, with no open issues returned.

## Runtime capability probe

Ran a separate ephemeral native Codex CLI in read-only mode with hooks disabled.
It was instructed to inspect its live tools and to launch a single smoke worker
only if it could also wait for and close that worker. It reported:

> CAPABILITY_BLOCKED: live delegation uses collaboration.spawn_agent and
> collaboration.wait_agent, with no native close-agent tool available.

No probe worker was launched. No goal, configuration, trust, application, Git,
or forge mutation was requested by this probe. An unrelated Postman connector
reported an authentication error during startup; that connector is not needed
for this project and was not modified.

The live current-session tools independently show the same gap. Completed agent
turns remain addressable for follow-up tasks; completion is not thread closure.

## User decisions

- Goals should support both Markdown milestone files and GitHub issues.
- The user explicitly selected a tested lifecycle adapter for this environment.
- Markdown can be the default entry source with explicit issue runs retained;
  this does not require conflating a milestone document and an issue snapshot.
- The user chose `development` as the integration target: per-change branches
  squash-merge there; release PRs from `development` to `main` preserve history
  through a normal merge. Remote inspection currently finds `main` only, so the
  integration branch must be established before opening feature PRs.
- The user delegated issue batch sizing to the agent to achieve the project goal.
  Use manageable batches (initially three issues) and continue as needed; do not
  reinterpret batch size as a limit on the overall goal. Start with one worker.
- Use inline PR review to match the requested PR/review flow. Keep automatic
  merging off and Figma integration disabled unless requested later.

The missing remote `development` branch was subsequently created at the
existing main commit `97d7ac064363c5944f40d86b14dbb0bf0c19177d`, with the approved
non-force Git push. No uncommitted application or harness work was published.

Before the first source checkpoint, `.codex` and `.agents` were made ignored
runtime installations, consistent with upstream scaffolding. The source lock,
installer and versioned adapter patch remain tracked inputs. This avoids losing
the operational helper tree when changing to a feature branch based on the
integration branch, and avoids publishing mutable progress/config/cache data.

## Adapter boundary

Implement before any upstream goal is active. Preserve close-based lifecycle as
the default and record observed completion separately from closure. Do not use
archive, interrupt, successful worker text, a timestamp, or a fabricated
`closed=true` flag as proof that a thread was closed.

Tests must cover concurrency, role evidence, terminal gates, and recovery using
the same lifecycle decision. Existing source/commit/run/rework isolation remains
required. Real runtime smoke evidence must be kept separate from synthetic
test fixtures.

The application remains at the already verified mock-window milestone. This
preflight does not retroactively convert that pilot into a full harness run.
