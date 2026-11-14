use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Button, Label, Orientation, glib};

const SAMPLE_LINES: [&str; 4] = [
    "Morning light across the desk,",
    "A quiet rhythm keeps me company.",
    "I leave a little room between the work and all the words I want to sing,",
    "And let the next line find its way to me.",
];

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

    let content = gtk::Box::new(Orientation::Vertical, 16);
    content.set_margin_top(20);
    content.set_margin_bottom(20);
    content.set_margin_start(20);
    content.set_margin_end(20);

    let heading = Label::builder()
        .label("Sample track — mock lyrics")
        .xalign(0.0)
        .wrap(true)
        .build();
    heading.add_css_class("title-3");
    content.append(&heading);

    let notice = Label::builder()
        .label("Demo only · use Previous and Next to change the current line.")
        .xalign(0.0)
        .wrap(true)
        .build();
    notice.add_css_class("dim-label");
    content.append(&notice);

    let lyrics = gtk::Box::new(Orientation::Vertical, 14);
    let labels: Rc<Vec<Label>> = Rc::new(
        SAMPLE_LINES
            .iter()
            .map(|_| {
                let label = Label::builder()
                    .xalign(0.0)
                    .hexpand(true)
                    .wrap(true)
                    .wrap_mode(gtk::pango::WrapMode::WordChar)
                    .build();
                lyrics.append(&label);
                label
            })
            .collect(),
    );
    let scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&lyrics)
        .build();
    content.append(&scroll);

    let selected = Rc::new(Cell::new(0));
    update_selection(&labels, selected.get());

    let controls = gtk::Box::new(Orientation::Horizontal, 8);
    controls.set_homogeneous(true);
    let previous = Button::with_label("Previous");
    let next = Button::with_label("Next");
    controls.append(&previous);
    controls.append(&next);
    content.append(&controls);

    let previous_selected = Rc::clone(&selected);
    let previous_labels = Rc::clone(&labels);
    previous.connect_clicked(move |_| {
        let index = (previous_selected.get() + SAMPLE_LINES.len() - 1) % SAMPLE_LINES.len();
        previous_selected.set(index);
        update_selection(&previous_labels, index);
    });
    next.connect_clicked(move |_| {
        let index = (selected.get() + 1) % SAMPLE_LINES.len();
        selected.set(index);
        update_selection(&labels, index);
    });

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Floating Lyrics — Demo")
        .default_width(480)
        .default_height(360)
        .resizable(true)
        .child(&content)
        .build();
    window.set_titlebar(Some(&gtk::HeaderBar::new()));
    window.present();
}

fn update_selection(labels: &[Label], selected: usize) {
    for (index, (label, text)) in labels.iter().zip(SAMPLE_LINES).enumerate() {
        if index == selected {
            let escaped = glib::markup_escape_text(text);
            label.set_markup(&format!("<b>{escaped}</b>"));
            label.remove_css_class("dim-label");
        } else {
            label.set_text(text);
            label.add_css_class("dim-label");
        }
    }
}
