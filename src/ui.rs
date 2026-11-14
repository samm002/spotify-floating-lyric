use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Button, Label, Orientation};

use crate::playback::{Availability, PlaybackSnapshot, PlaybackStatus};

pub const SAMPLE_LINES: [&str; 4] = [
    "Morning light across the desk,",
    "A quiet rhythm keeps me company.",
    "I leave a little room between the work and all the words I want to sing,",
    "And let the next line find its way to me.",
];

/// The shared app content used by the desktop window and render example.
pub struct PlaybackUi {
    root: gtk::Box,
    title: Label,
    artists: Label,
    status: Label,
    position: Label,
    selected_line: Rc<Cell<usize>>,
    previous: Button,
    next: Button,
}

impl PlaybackUi {
    pub fn new() -> Self {
        let root = gtk::Box::new(Orientation::Vertical, 10);
        root.set_margin_top(16);
        root.set_margin_bottom(16);
        root.set_margin_start(16);
        root.set_margin_end(16);

        let app_heading = Label::builder()
            .label("Spotify Floating Lyric")
            .xalign(0.0)
            .build();
        app_heading.add_css_class("title-3");
        root.append(&app_heading);

        let playback_content = gtk::Box::new(Orientation::Vertical, 3);
        playback_content.set_margin_top(8);
        playback_content.set_margin_bottom(8);
        playback_content.set_margin_start(10);
        playback_content.set_margin_end(10);

        let title = wrapped_label("Connecting to Spotify…");
        title.add_css_class("title-4");
        playback_content.append(&title);

        let artists = wrapped_label("");
        artists.add_css_class("dim-label");
        playback_content.append(&artists);

        let playback_details = gtk::Box::new(Orientation::Horizontal, 12);
        let status = wrapped_label("Waiting for Spotify");
        status.set_hexpand(true);
        let position = Label::builder()
            .label("Position unavailable")
            .xalign(1.0)
            .wrap(true)
            .build();
        position.add_css_class("dim-label");
        playback_details.append(&status);
        playback_details.append(&position);
        playback_content.append(&playback_details);

        let playback_frame = gtk::Frame::new(Some("Spotify playback"));
        playback_frame.set_child(Some(&playback_content));
        root.append(&playback_frame);

        let mock_heading = Label::builder()
            .label("Sample track — mock lyrics")
            .xalign(0.0)
            .wrap(true)
            .build();
        mock_heading.add_css_class("heading");
        root.append(&mock_heading);

        let notice = wrapped_label(
            "Original demo text is unrelated to Spotify playback. Previous and Next change the highlighted line.",
        );
        notice.add_css_class("dim-label");
        root.append(&notice);

        let lyrics = gtk::Box::new(Orientation::Vertical, 12);
        let sample_labels: Rc<Vec<Label>> = Rc::new(
            SAMPLE_LINES
                .iter()
                .map(|_| {
                    let label = wrapped_label("");
                    label.set_hexpand(true);
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
        root.append(&scroll);

        let selected_line = Rc::new(Cell::new(0));
        update_selection(&sample_labels, selected_line.get());

        let controls = gtk::Box::new(Orientation::Horizontal, 8);
        controls.set_homogeneous(true);
        let previous = Button::with_label("Previous");
        let next = Button::with_label("Next");
        controls.append(&previous);
        controls.append(&next);
        root.append(&controls);

        let previous_selected = Rc::clone(&selected_line);
        let previous_labels = Rc::clone(&sample_labels);
        previous.connect_clicked(move |_| {
            let index = (previous_selected.get() + SAMPLE_LINES.len() - 1) % SAMPLE_LINES.len();
            previous_selected.set(index);
            update_selection(&previous_labels, index);
        });
        let next_selected = Rc::clone(&selected_line);
        let next_labels = Rc::clone(&sample_labels);
        next.connect_clicked(move |_| {
            let index = (next_selected.get() + 1) % SAMPLE_LINES.len();
            next_selected.set(index);
            update_selection(&next_labels, index);
        });

        Self {
            root,
            title,
            artists,
            status,
            position,
            selected_line,
            previous,
            next,
        }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    pub fn update_playback(&self, snapshot: &PlaybackSnapshot) {
        match &snapshot.availability {
            Availability::Connecting => {
                self.title.set_text("Connecting to Spotify…");
                self.artists.set_text("");
                self.status.set_text("Waiting for Spotify");
                self.position.set_text("Position unavailable");
            }
            Availability::Disconnected => {
                self.title.set_text("Spotify is not running");
                self.artists
                    .set_text("Start Spotify to show current playback information.");
                self.status.set_text("Waiting for Spotify playback");
                self.position.set_text("Position unavailable");
            }
            Availability::Error(error) => {
                self.title.set_text("Spotify connection unavailable");
                self.artists.set_text("");
                self.status
                    .set_text(&format!("Could not read Spotify playback: {error}"));
                self.position.set_text("Position unavailable");
            }
            Availability::Connected => {
                self.title
                    .set_text(snapshot.title.as_deref().unwrap_or("Unknown track"));
                let artist_text = if snapshot.artists.is_empty() {
                    "Unknown artist".to_owned()
                } else {
                    snapshot.artists.join(" · ")
                };
                self.artists.set_text(&artist_text);
                self.status.set_text(status_text(snapshot.status));
                self.position
                    .set_text(&format_position(snapshot.position_us, snapshot.duration_us));
            }
        }
    }

    /// Emit the actual button signal so the render helper can check wrapping.
    pub fn click_previous_demo_line(&self) {
        self.previous.emit_clicked();
    }

    /// Emit the actual button signal so the render helper can check wrapping.
    pub fn click_next_demo_line(&self) {
        self.next.emit_clicked();
    }

    pub fn selected_demo_line(&self) -> usize {
        self.selected_line.get()
    }
}

impl Default for PlaybackUi {
    fn default() -> Self {
        Self::new()
    }
}

fn wrapped_label(text: &str) -> Label {
    Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .build()
}

fn status_text(status: PlaybackStatus) -> &'static str {
    match status {
        PlaybackStatus::Playing => "Playing",
        PlaybackStatus::Paused => "Paused",
        PlaybackStatus::Stopped => "Stopped",
        PlaybackStatus::Unknown => "Playback status unavailable",
    }
}

fn format_position(position_us: Option<u64>, duration_us: Option<u64>) -> String {
    let Some(position_us) = position_us else {
        return "Position unavailable".to_owned();
    };
    let position = format_elapsed(position_us);
    match duration_us {
        Some(duration_us) => format!("{position} / {}", format_elapsed(duration_us)),
        None => format!("{position} elapsed"),
    }
}

fn format_elapsed(microseconds: u64) -> String {
    let total_seconds = microseconds / 1_000_000;
    let seconds = total_seconds % 60;
    let total_minutes = total_seconds / 60;
    if total_minutes >= 60 {
        format!(
            "{}:{:02}:{seconds:02}",
            total_minutes / 60,
            total_minutes % 60
        )
    } else {
        format!("{total_minutes}:{seconds:02}")
    }
}

fn update_selection(labels: &[Label], selected: usize) {
    for (index, (label, text)) in labels.iter().zip(SAMPLE_LINES).enumerate() {
        if index == selected {
            let escaped = gtk::glib::markup_escape_text(text);
            label.set_markup(&format!("<b>{escaped}</b>"));
            label.remove_css_class("dim-label");
        } else {
            label.set_text(text);
            label.add_css_class("dim-label");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{format_elapsed, format_position};

    #[test]
    fn elapsed_time_shows_minutes_and_hours() {
        assert_eq!(format_elapsed(0), "0:00");
        assert_eq!(format_elapsed(65_000_000), "1:05");
        assert_eq!(format_elapsed(3_661_000_000), "1:01:01");
    }

    #[test]
    fn position_shows_duration_only_when_known() {
        assert_eq!(
            format_position(None, Some(120_000_000)),
            "Position unavailable"
        );
        assert_eq!(
            format_position(Some(65_000_000), Some(120_000_000)),
            "1:05 / 2:00"
        );
        assert_eq!(format_position(Some(65_000_000), None), "1:05 elapsed");
    }
}
