# Spotify Floating Lyric

A Rust + GTK4 desktop app for Ubuntu GNOME. It observes Spotify playback through
MPRIS and shows the current title, artists, playback status, and elapsed
position. The original four lines of sample text remain as an independent mock
lyrics demonstration; they are unrelated to the current Spotify track.

The app does not fetch lyrics or control Spotify. It keeps a normal resizable
window with GNOME decorations and does not automatically pin itself, click
through windows, or move across workspaces.

## Build prerequisites

On Ubuntu, install Rust, Cargo, and GTK4 development files:

```sh
sudo apt-get install rustc cargo rustfmt rust-clippy libgtk-4-dev
```

The project declares Rust 1.93 and uses GTK4's re-exported GIO APIs for MPRIS.
Cargo.lock is committed; keep it with the application source.

## Run the app

```sh
cargo run --locked
```

If Spotify is stopped, the playback section says it is not running. When
Spotify appears on the session bus, the app displays its current state and
clears it again when Spotify exits. Position updates while playback is active;
the elapsed value includes the duration when Spotify provides one. Missing
track fields are shown as unknown. The mock sample remains available in every
playback state, and Previous/Next wrap around its four lines.

The current Spotify title and artists are rendered as plain text. The sample
heading and notice keep that text separate from Spotify metadata.

## Keep the window visible

Use GNOME's ordinary title-bar menu or **Alt+Space** to pin the window manually
if the desktop offers that option. The app does not promise behavior above
fullscreen windows or across workspaces, and does not implement transparency
or click-through.

## Verify the source

Run the repository verification and harness checks:

```sh
bash scripts/verify.sh
python3 scripts/test-harness.py
```

The focused UI time-format tests can be run with:

```sh
cargo test --locked --lib ui::tests
```

The fake MPRIS service protocol test uses an isolated session bus:

```sh
dbus-run-session -- cargo test --locked --test mpris_protocol -- --test-threads=1
```

Compilation, unit tests, and fake-service protocol tests do not establish that
the app behaves correctly with a running Spotify installation or GNOME's
window manager.

## Render the GTK content

On a graphical session, render the shared app UI into PNG files:

```sh
cargo run --locked --example render_playback
```

The helper captures disconnected, playing with long metadata, paused, and
narrow-window states under `target/playback-renders/`. It checks that the
Previous and Next button signals preserve selection and wrap between the first
and last sample lines. The helper waits for GTK layout and frame readiness,
then uses `WidgetPaintable`, `Snapshot`, GSK, and a PNG texture export. It exits
with an error if layout, rendering, or writing a file fails.

To also capture the live observer's current view from the session bus, run:

```sh
cargo run --locked --example render_playback -- --live
```

This still writes the synthetic captures and adds `live.png`. The live image
uses whatever Spotify reports at capture time. Rendered content can be reviewed
for wrapping and clipping, but does not show compositor decorations, focus,
window-menu behavior, pinning, or cross-workspace behavior.

## Check against actual Spotify

On Ubuntu GNOME with Spotify installed, run `cargo run --locked` and observe:

1. Launch the app while Spotify is stopped and confirm its waiting state.
2. Start Spotify and confirm title, artists, status, and position appear.
3. Change tracks; confirm metadata changes and elapsed position resets.
4. Pause and resume; confirm status changes and position stays still while paused.
5. Seek in Spotify and confirm the displayed position follows.
6. Close Spotify and confirm its metadata and moving position clear.
7. Restart Spotify and confirm the app reacquires the current state.
8. Resize the window; check wrapping, mock Previous/Next behavior, and normal
   GNOME window decorations.
9. Close the app normally and confirm its process exits.

Record live Spotify and desktop observations separately from the isolated fake
service test and GTK render images. If Spotify or GNOME is unavailable, report
those desktop observations as not run rather than inferring them from a render.
