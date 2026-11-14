# Milestone 003: Spotify playback through MPRIS

## Epic: Spotify playback through MPRIS

Observe Spotify playback through its MPRIS session-bus service and show the
current track state alongside the existing original mock-text demonstration.
The application observes Spotify; it does not control playback or imply that
the demonstration text belongs to the current track.

### Acceptance

- The app detects Spotify when it is already running or starts later, reports
  an absent player clearly, clears old state on owner loss, and reacquires fresh
  state after Spotify restarts.
- The display shows title, all available artists, playback status, and elapsed
  position; it shows duration when Spotify supplies a valid duration.
- Track changes, pause/resume, seeks, and periodic position updates are reflected
  without relying on `Position` property-change notifications.
- Missing or malformed MPRIS metadata is handled safely, and delayed replies from
  a previous owner or track cannot overwrite newer playback state.
- The four original mock lines and Previous/Next selection controls remain
  independently usable. The text stays identified as demonstration text and is
  never replaced by real song lyrics.
- The window remains resizable with normal GTK/GNOME decorations, readable text
  wrapping, a clear disconnected state, and no automatic window-management
  behavior.
- Automated coverage includes state transitions and a real GIO D-Bus adapter
  exercised against an isolated fake MPRIS service. A documented desktop
  checklist distinguishes synthetic-bus, rendered GTK, and actual Spotify
  observations.
- `bash scripts/verify.sh` and `python3 scripts/test-harness.py` results are
  recorded accurately. A render or unit test is not treated as proof of live
  Spotify or GNOME window-manager behavior.

### Tasks

- [ ] Discover Spotify on MPRIS and recover after owner loss
- [ ] Track metadata, playback status and seek position
- [ ] Integrate playback display and verify the preserved mock demo

### Task boundaries and implementation notes

#### Discover Spotify on MPRIS and recover after owner loss

Use GTK's re-exported GIO APIs to watch the session-bus name
`org.mpris.MediaPlayer2.spotify` and its `/org/mpris/MediaPlayer2` object. Do not
launch Spotify when it is absent. Handle absent-at-startup, appearance, owner
loss, bus errors, and reacquisition after a restart. Clear the prior playback
state immediately when its owner disappears. Bound asynchronous calls and tag
each owner session with a generation so a delayed reply from an old owner is
discarded. Re-read the complete initial state whenever a new owner is acquired.

Tests cover absent → acquired → lost → reacquired transitions and prove that a
delayed old-owner response cannot restore stale state. Exercise owner changes
through GIO against a registered fake service on an isolated session bus; a
pure-state test alone does not establish D-Bus signal wiring.

#### Track metadata, playback status and seek position

Decode `Metadata` defensively. Accept a string title and the MPRIS string-array
artist value, joining all valid artists for display. Missing, wrongly typed, or
otherwise malformed values produce safe unknown values and never panic. Treat
track object identity as optional. Treat duration as optional and display it
only when its MPRIS value is valid.

Represent position and duration as MPRIS microseconds at the protocol boundary,
converting to elapsed display time in the UI. Reject negative or wrongly typed
positions safely; do not invent a duration. Reset position on an accepted track
change, take `Seeked` and fresh property replies as authoritative, and poll
`Position` periodically while playing because it is not required to emit
`PropertiesChanged`. Freeze the last accepted position while paused. If a valid
duration exists, keep the shown elapsed value within that duration. Ignore
pending reads associated with a superseded owner or track.

Tests cover multi-artist decoding, absent and malformed properties, optional
duration, negative position, metadata invalidation/reread, pause/resume,
`Seeked`, periodic reads in the absence of position notifications, and a track
switch while a position read is pending. Isolated-bus assertions must time out
as failures rather than silently skip.

#### Integrate playback display and verify the preserved mock demo

Add a compact playback display for title, artists, status, elapsed position and
optional duration. Keep the original four demonstration lines and Previous/Next
controls available regardless of Spotify state. Label the text as a mock
demonstration so current metadata cannot be mistaken for synchronized lyrics.
Render external metadata as plain text. Keep the existing normal header bar and
resizable layout; do not add playback controls or seek sliders.

Provide a documented, opt-in GTK render helper that uses the real shared UI with
synthetic playback states. It must wait for layout/frame readiness, render the
widget to PNG, and support disconnected, playing with long metadata, paused,
and narrow-window states. Inspect the produced images for clipping, wrapping,
and legibility. Generated images belong in an ignored artifact directory.
Rendered GTK content does not establish compositor decorations or other desktop
window-management behavior.

### Verification and evidence

Run `bash scripts/verify.sh` for formatting, locked build, Clippy, and Rust
tests, and run `PATH="$PWD/.tools/bin:$PATH" python3 scripts/test-harness.py`
for harness QA. Run the D-Bus protocol tests inside `dbus-run-session` with a
single test thread so they use an isolated session bus. The implementation must
document the exact focused test and render-helper commands it provides.

Record protocol tests, inspected rendered GTK images, and live desktop
observations as separate evidence. A fake MPRIS service proves protocol wiring
against the tested service shape; it does not prove behavior with Spotify. A
rendered widget proves content layout only; it does not prove GNOME decorations,
focus, pinning, click-through, or cross-workspace behavior. Record unavailable
checks as NOT_RUN or UNKNOWN with the concrete reason, never as PASS.

### Actual Spotify desktop checklist

On an Ubuntu GNOME session with Spotify installed, record the session and app
version, then perform these observations separately from automated checks:

1. Launch the app while Spotify is stopped and confirm the disconnected state.
2. Start Spotify and confirm fresh title, artists, status, and position appear.
3. Change tracks and confirm metadata and elapsed position reset for the new
   track.
4. Pause and resume Spotify; confirm status changes and elapsed position stays
   still while paused, then continues after resume.
5. Seek in Spotify and confirm the displayed position follows the seek.
6. Close Spotify and confirm its metadata and moving position are cleared.
7. Restart Spotify and confirm the app reacquires its current state.
8. Resize the lyric window, verify wrapping and the original mock controls, and
   confirm normal window decorations remain available.
9. Close the app normally and confirm the process exits.

If Spotify or a GNOME session is unavailable, record those desktop steps as not
run. Synthetic bus and render evidence cannot substitute for these observations.

### Out of scope

Lyric fetching, real lyric fixtures, playback controls, automatic pinning,
click-through, cross-workspace behavior, packaging, and release are outside this
milestone. Manual use of ordinary GNOME window controls remains available.
