# Milestone 002: lifecycle compatibility before goal activation

## User-approved scope

Adapt the pinned harness to the actual collaboration API, which exposes terminal
worker status but no close-agent action. Develop and test the adapter before any
real project goal is active. Preserve native close-based behavior by default.

The user also established the branch flow: per-change PRs target `development`
and squash-merge; release PRs go from `development` to `main` without squashing.
Automatic merging remains disabled.

## Implementation contract

- Freeze `native_close` or opt-in `terminal_observation` on each reservation.
  Existing reservations without a mode retain native semantics.
- Generate a unique worker task name per observation-mode reservation and bind
  the confirmed canonical agent name to it. Never reuse a released worker.
- Accept an observation file containing a saved real `collaboration.list_agents`
  result plus exact reservation, goal, run, group, task, cycle, commit and time
  identity. Persist validated contents in state, not just the file path.
- Require exactly one matching canonical worker with a terminal completed
  result. Reject missing, running, unknown, duplicate, replayed, stale or
  mismatched observations atomically. No timestamp-only release.
- Keep `closed=false` for observed completion. A semantic task failure may end
  a completed worker turn but must not satisfy a successful-role gate.
- Reuse centralized release/success predicates in concurrency, separate role
  evidence helpers, task DONE, implementation readiness, and terminal completion.
- Retry/cancel/reap cannot free an unobserved live observation-mode worker.
  A known failed launch without a worker may release through launch evidence.
- Preserve assignment, current-SHA and rework isolation and native closure
  requirements. Configuration changes cannot reinterpret existing reservations.

## Branch policy guard

The upstream GitHub merge helper currently hardcodes a normal merge. Add a small
opt-in project branch-flow selector: development targets choose squash; main
targets require development as the head and choose normal merge. Legacy projects
without that policy retain existing behavior. Test arguments with a mocked forge;
do not perform a real merge during development or tests.

## Roles and ownership

- Planner: map all callers and recovery invariants (read-only; completed).
- Builder: stage protected helpers and skill updates under
  `/tmp/spotify-lifecycle-stage`; own the reproducible patch, installer upgrade
  support, and regression tests in the project. No app source changes.
- MAIN: coordinate, run independent checks and a real isolated smoke, record
  evidence and install the reviewed staged changes with required permissions.
- Reviewer: inspect predicates, failure/recovery coverage, installer behavior,
  skill instructions and branch-policy compatibility independently.

## Acceptance

1. Meaningful isolated tests cover native defaults, frozen modes, all observation
   identity/status/time mismatches, concurrency release without closure, semantic
   failure, recovery restrictions, replay/reuse, stale SHA/rework evidence and
   merge argument selection/rejection.
2. Installation verifies the upstream archive and versioned patch checksums,
   reproduces the reviewed bytes, and refuses arbitrary customization overwrite.
3. Skill validation passes and the documented launch/finish recipe uses the live
   API. Observation mode embeds role instructions and requires no global trust
   edit or fictitious close call.
4. Independent review has no unresolved actionable findings.
5. A fresh real worker completes in an isolated test repository; the actual
   observed status releases its reservation with `closed=false` throughout.

Synthetic fixture successes are not real worker evidence. Do not retroactively
claim the mock-window local pilot as a full harness run. Commit/PR delivery and
activation of the real application goal follow bootstrap readiness separately.
