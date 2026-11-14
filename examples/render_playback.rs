use std::cell::{Cell, RefCell};
use std::error::Error;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, glib};
use spotify_floating_lyric::playback::{
    Availability, PlaybackObserver, PlaybackSnapshot, PlaybackStatus,
};
use spotify_floating_lyric::ui::PlaybackUi;

const OUTPUT_DIR: &str = "target/playback-renders";
const FRAME_READY_DELAY: Duration = Duration::from_millis(250);
const RENDER_TIMEOUT: Duration = Duration::from_secs(5);
const LIVE_CAPTURE_DELAY: Duration = Duration::from_secs(2);

struct Scenario {
    name: &'static str,
    width: i32,
    height: i32,
    snapshot: PlaybackSnapshot,
}

struct RenderRun {
    app: Application,
    window: Option<ApplicationWindow>,
    ui: Option<Rc<PlaybackUi>>,
    hold_guard: Option<gtk::gio::ApplicationHoldGuard>,
    scenarios: Vec<Scenario>,
    index: usize,
    active_scenario: Option<usize>,
    ready_since: Option<Instant>,
    scenario_started: Instant,
    output_dir: PathBuf,
    live_enabled: bool,
    live_observer: Option<PlaybackObserver>,
    live_started: Option<Instant>,
    live_ready_since: Option<Instant>,
    live_capture_done: bool,
    completed: Rc<Cell<bool>>,
    error: Rc<RefCell<Option<String>>>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let live_enabled = std::env::args().any(|arg| arg == "--live");
    std::fs::create_dir_all(OUTPUT_DIR)?;

    let error = Rc::new(RefCell::new(None));
    let completed = Rc::new(Cell::new(false));
    let app = Application::builder()
        .application_id("io.github.samm002.SpotifyFloatingLyric.Render")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    let app_error = Rc::clone(&error);
    let app_completed = Rc::clone(&completed);
    app.connect_activate(move |app| {
        let run = Rc::new(RefCell::new(RenderRun {
            app: app.clone(),
            window: None,
            ui: None,
            hold_guard: Some(app.hold()),
            scenarios: synthetic_scenarios(),
            index: 0,
            active_scenario: None,
            ready_since: None,
            scenario_started: Instant::now(),
            output_dir: PathBuf::from(OUTPUT_DIR),
            live_enabled,
            live_observer: None,
            live_started: None,
            live_ready_since: None,
            live_capture_done: false,
            completed: Rc::clone(&app_completed),
            error: Rc::clone(&app_error),
        }));
        glib::timeout_add_local(Duration::from_millis(50), {
            let run = Rc::clone(&run);
            move || advance_render(&run)
        });
    });

    let args = std::env::args().take(1).collect::<Vec<_>>();
    app.run_with_args(&args);
    if let Some(error) = error.borrow_mut().take() {
        return Err(error.into());
    }
    if !completed.get() {
        return Err("GTK application exited before all requested renders completed".into());
    }
    Ok(())
}

fn check_mock_controls(ui: &PlaybackUi) -> Result<(), String> {
    if ui.selected_demo_line() != 0 {
        return Err("mock selection did not start on the first line".to_owned());
    }
    ui.click_previous_demo_line();
    if ui.selected_demo_line() != 3 {
        return Err("Previous did not wrap from the first line to the last".to_owned());
    }
    ui.click_next_demo_line();
    if ui.selected_demo_line() != 0 {
        return Err("Next did not wrap from the last line to the first".to_owned());
    }
    ui.click_next_demo_line();
    if ui.selected_demo_line() != 1 {
        return Err("Next did not select the following mock line".to_owned());
    }
    ui.click_previous_demo_line();
    if ui.selected_demo_line() != 0 {
        return Err("Previous did not return to the first mock line".to_owned());
    }
    Ok(())
}

fn advance_render(run: &Rc<RefCell<RenderRun>>) -> glib::ControlFlow {
    let mut state = run.borrow_mut();
    if state.index < state.scenarios.len() {
        let index = state.index;
        if state.active_scenario != Some(index) {
            let scenario = &state.scenarios[index];
            let ui = Rc::new(PlaybackUi::new());
            if index == 0
                && let Err(error) = check_mock_controls(&ui)
            {
                fail(&state.app, &state.error, error);
                return glib::ControlFlow::Break;
            }
            ui.update_playback(&scenario.snapshot);
            let window = build_window(&state.app, ui.widget(), scenario.width, scenario.height);
            state.ui = Some(ui);
            state.window = Some(window);
            state.active_scenario = Some(index);
            state.ready_since = None;
            state.scenario_started = Instant::now();
        }

        if state.scenario_started.elapsed() > RENDER_TIMEOUT {
            let name = state.scenarios[index].name;
            fail(
                &state.app,
                &state.error,
                format!("GTK layout for {name} did not become ready within {RENDER_TIMEOUT:?}"),
            );
            return glib::ControlFlow::Break;
        }

        let ui = state.ui.as_ref().expect("scenario UI exists").clone();
        let window = state
            .window
            .as_ref()
            .expect("scenario window exists")
            .clone();
        let width = ui.widget().allocated_width();
        let height = ui.widget().allocated_height();
        if width > 1 && height > 1 && window.surface().is_some() {
            let ready_since = state.ready_since.get_or_insert_with(Instant::now);
            if ready_since.elapsed() >= FRAME_READY_DELAY {
                let scenario_name = state.scenarios[index].name;
                let path = state.output_dir.join(format!("{scenario_name}.png"));
                match capture_widget(&window, &path) {
                    Ok((captured_width, captured_height)) => {
                        if scenario_name == "narrow" && captured_width > 360 {
                            fail(
                                &state.app,
                                &state.error,
                                format!(
                                    "narrow render was {captured_width}px wide; expected at most 360px"
                                ),
                            );
                            return glib::ControlFlow::Break;
                        }
                        println!(
                            "captured {}: {}x{} -> {}",
                            scenario_name,
                            captured_width,
                            captured_height,
                            path.display()
                        );
                        window.close();
                        state.window = None;
                        state.ui = None;
                        state.index += 1;
                        state.active_scenario = None;
                        state.ready_since = None;
                    }
                    Err(error) => {
                        fail(&state.app, &state.error, error);
                        return glib::ControlFlow::Break;
                    }
                }
            }
        } else {
            state.ready_since = None;
        }
        return glib::ControlFlow::Continue;
    }

    if state.live_enabled && !state.live_capture_done {
        if state.live_observer.is_none() {
            let ui = Rc::new(PlaybackUi::new());
            let window = build_window(&state.app, ui.widget(), 480, 440);
            let weak_ui = Rc::downgrade(&ui);
            state.live_observer = Some(PlaybackObserver::new(move |snapshot| {
                if let Some(ui) = weak_ui.upgrade() {
                    ui.update_playback(&snapshot);
                }
            }));
            state.window = Some(window);
            state.ui = Some(ui);
            state.live_started = Some(Instant::now());
            return glib::ControlFlow::Continue;
        }

        if state
            .live_started
            .is_some_and(|started| started.elapsed() > LIVE_CAPTURE_DELAY + RENDER_TIMEOUT)
        {
            fail(
                &state.app,
                &state.error,
                "live observer view did not become renderable before timeout".to_owned(),
            );
            return glib::ControlFlow::Break;
        }

        if state
            .live_started
            .is_some_and(|started| started.elapsed() >= LIVE_CAPTURE_DELAY)
        {
            let ui = state.ui.as_ref().expect("live UI exists").clone();
            let window = state.window.as_ref().expect("live window exists").clone();
            let width = ui.widget().allocated_width();
            let height = ui.widget().allocated_height();
            if width > 1 && height > 1 && window.surface().is_some() {
                let ready_since = state.live_ready_since.get_or_insert_with(Instant::now);
                if ready_since.elapsed() >= FRAME_READY_DELAY {
                    let path = state.output_dir.join("live.png");
                    match capture_widget(&window, &path) {
                        Ok((width, height)) => {
                            println!(
                                "captured live observer view: {width}x{height} -> {}",
                                path.display()
                            );
                            state.live_observer.take();
                            state.live_capture_done = true;
                            window.close();
                            state.window = None;
                            state.ui = None;
                        }
                        Err(error) => {
                            fail(&state.app, &state.error, error);
                            return glib::ControlFlow::Break;
                        }
                    }
                }
            }
        }
        return glib::ControlFlow::Continue;
    }

    state.live_observer.take();
    if let Some(window) = state.window.take() {
        window.close();
    }
    state.ui.take();
    state.hold_guard.take();
    state.completed.set(true);
    state.app.quit();
    glib::ControlFlow::Break
}

fn build_window(
    app: &Application,
    content: &impl IsA<gtk::Widget>,
    width: i32,
    height: i32,
) -> ApplicationWindow {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Spotify Floating Lyric render helper")
        .default_width(width)
        .default_height(height)
        .resizable(true)
        .child(content)
        .build();
    window.set_titlebar(Some(&gtk::HeaderBar::new()));
    window.present();
    window
}

fn capture_widget(
    window: &ApplicationWindow,
    path: &std::path::Path,
) -> Result<(i32, i32), String> {
    let width = window.allocated_width();
    let height = window.allocated_height();
    let surface = window
        .surface()
        .ok_or_else(|| "GTK window has no render surface".to_owned())?;
    let renderer = gtk::gsk::Renderer::for_surface(&surface)
        .ok_or_else(|| "GSK could not create a renderer for the GTK surface".to_owned())?;
    let result = (|| {
        let paintable = gtk::WidgetPaintable::new(Some(window));
        let snapshot = gtk::Snapshot::new();
        paintable.snapshot(&snapshot, width as f64, height as f64);
        let node = snapshot
            .to_node()
            .ok_or_else(|| "GTK window produced an empty render node".to_owned())?;
        let viewport = gtk::graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
        let texture = renderer.render_texture(node, Some(&viewport));
        if texture.width() != width || texture.height() != height {
            return Err(format!(
                "GSK returned a {}x{} texture for a {width}x{height} window",
                texture.width(),
                texture.height()
            ));
        }
        texture
            .save_to_png(path)
            .map_err(|error| format!("could not write {}: {error}", path.display()))?;
        Ok((width, height))
    })();
    renderer.unrealize();
    result
}

fn fail(app: &Application, error_slot: &Rc<RefCell<Option<String>>>, error: String) {
    eprintln!("render helper failed: {error}");
    *error_slot.borrow_mut() = Some(error);
    app.quit();
}

fn synthetic_scenarios() -> Vec<Scenario> {
    let disconnected = PlaybackSnapshot {
        availability: Availability::Disconnected,
        ..PlaybackSnapshot::default()
    };
    let playing = PlaybackSnapshot {
        availability: Availability::Connected,
        title: Some("A Long Example Track Title Used to Check Narrow Window Wrapping".to_owned()),
        artists: vec![
            "Example Artist One with a Longer Name".to_owned(),
            "Example Artist Two".to_owned(),
        ],
        status: PlaybackStatus::Playing,
        position_us: Some(87_000_000),
        duration_us: Some(252_000_000),
        track_id: Some("/example/track/one".to_owned()),
    };
    let paused = PlaybackSnapshot {
        status: PlaybackStatus::Paused,
        position_us: Some(121_000_000),
        ..playing.clone()
    };

    vec![
        Scenario {
            name: "disconnected",
            width: 480,
            height: 440,
            snapshot: disconnected,
        },
        Scenario {
            name: "playing-long-metadata",
            width: 480,
            height: 440,
            snapshot: playing,
        },
        Scenario {
            name: "paused",
            width: 480,
            height: 440,
            snapshot: paused.clone(),
        },
        Scenario {
            name: "narrow",
            width: 320,
            height: 560,
            snapshot: paused,
        },
    ]
}
