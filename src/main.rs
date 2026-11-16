use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, glib};
use spotify_floating_lyric::playback::PlaybackObserver;
use spotify_floating_lyric::ui::PlaybackUi;

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id("io.github.samm002.SpotifyFloatingLyric")
        .build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    if let Some(window) = app.active_window() {
        window.present();
        return;
    }

    let ui = Rc::new(PlaybackUi::new());
    let weak_ui = Rc::downgrade(&ui);
    let observer = PlaybackObserver::new(move |snapshot| {
        if let Some(ui) = weak_ui.upgrade() {
            ui.update_playback(&snapshot);
        }
    });
    let observer = Rc::new(RefCell::new(Some(observer)));

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Spotify Floating Lyric")
        .default_width(480)
        .default_height(440)
        .resizable(true)
        .child(ui.widget())
        .build();
    window.set_titlebar(Some(&gtk::HeaderBar::new()));

    // The observer callback only holds a Weak UI reference. The window keeps
    // the view and observer alive together, then drops the observer on close.
    let ui_keepalive = Rc::clone(&ui);
    window.connect_close_request(move |_| {
        observer.borrow_mut().take();
        let _ = ui_keepalive.widget();
        glib::Propagation::Proceed
    });

    window.present();
}
