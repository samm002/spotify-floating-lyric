# Spotify Floating Lyric

A Rust + GTK4 desktop app for Ubuntu GNOME. The first milestone is a **mock
lyrics window**: it shows original sample text and lets you select the current
line with Previous and Next. It does not connect to Spotify or fetch lyrics yet.

The project also teaches agent harness development through bounded planning,
delegated implementation, executable checks, independent review, and recorded
desktop observations. Start with [the harness bootstrap](docs/harness/bootstrap.md)
and [milestone 001](docs/plans/001-mock-window.md).

## Build prerequisites

On Ubuntu 26.04, install the compiler, Cargo tools and GTK4 development files in
your terminal:

```sh
sudo apt-get install rustc cargo rustfmt rust-clippy libgtk-4-dev
```

The app declares Rust 1.93 and uses gtk-rs GTK4 bindings. Build dependencies are
downloaded by Cargo on the first build. See the [gtk-rs introduction](https://gtk-rs.org/gtk4-rs/stable/latest/book/hello_world.html)
for the application lifecycle used here.

## Run the demo

For the initial bootstrap, generate the application lockfile if it is not yet
present, then build and run:

```sh
cargo generate-lockfile
cargo run --locked
```

Retain and commit Cargo.lock. Once it exists, normal runs should use
`cargo run --locked` without regenerating it. The normal title bar provides the
close control, the window can be resized, and the sample lines wrap. Previous
and Next cycle through the four lines, emphasizing the selected line.

## Keep the window visible

On GNOME Wayland, open the window menu from the title bar or with **Alt+Space**,
then select **Always on Top** if offered. Switch to your editor or browser to
check the behavior on your laptop. The app keeps a normal header bar for this
interaction; it does not programmatically pin itself.

The initial milestone does not promise behavior above fullscreen windows or
across workspaces, and does not implement transparency or click-through.

## Verify

After dependencies and Cargo.lock are available:

```sh
bash scripts/verify.sh
```

This checks formatting, a locked build, Clippy with warnings denied, and the
available tests. The initial demo has no automated UI tests. Compilation and
linting do not establish that the window launches, closes, resizes correctly, or
stays above other apps: follow and record the desktop checklist in milestone 001.
