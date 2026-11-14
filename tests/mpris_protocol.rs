use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::gio;
use gtk::glib;
use gtk::glib::variant::ToVariant;
use spotify_floating_lyric::playback::{
    Availability, PlaybackObserver, PlaybackSnapshot, PlaybackStatus,
};

const SERVICE: &str = "org.mpris.MediaPlayer2.spotify";
const OBJECT_PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const DBUS: &str = "org.freedesktop.DBus";
const DBUS_PATH: &str = "/org/freedesktop/DBus";
const WAIT: Duration = Duration::from_secs(8);

const PLAYER_XML: &str = r#"
<node>
  <interface name="org.mpris.MediaPlayer2.Player">
    <property name="PlaybackStatus" type="s" access="read"/>
    <property name="Metadata" type="a{sv}" access="read"/>
    <property name="Position" type="x" access="read"/>
    <signal name="Seeked"><arg name="Position" type="x"/></signal>
  </interface>
</node>
"#;

#[test]
fn observes_owner_properties_seek_polling_and_replacement_on_private_bus() {
    let bus = PrivateBus::start();
    let observer_connection = bus.connect();
    let latest = Rc::new(RefCell::new(PlaybackSnapshot::default()));
    let callback_latest = Rc::clone(&latest);
    let history = Rc::new(RefCell::new(Vec::<PlaybackSnapshot>::new()));
    let callback_history = Rc::clone(&history);
    let observer = PlaybackObserver::from_connection(observer_connection, move |snapshot| {
        *callback_latest.borrow_mut() = snapshot.clone();
        callback_history.borrow_mut().push(snapshot);
    });

    wait_for("absent Spotify is reported", || {
        latest.borrow().availability == Availability::Disconnected
    });

    let mut first =
        FakeMpris::claim_without_object(&bus, "First original title", "Playing", 4_000_000);
    wait_for("failed initial GetAll is surfaced", || {
        matches!(latest.borrow().availability, Availability::Error(_))
    });
    first.change_metadata_on_next_read("Updated before initial reply");
    first.install_player_object();
    wait_for("first owner and initial state are acquired", || {
        let snapshot = latest.borrow();
        snapshot.availability == Availability::Connected
            && snapshot.title.as_deref() == Some("Updated before initial reply")
            && snapshot.artists == ["Artist One", "Artist Two"]
            && snapshot.status == PlaybackStatus::Playing
            && snapshot.position_us == Some(4_000_000)
            && snapshot.duration_us == Some(120_000_000)
    });

    // No Position change signal is sent. The observer must make periodic
    // authoritative reads while playing.
    let reads_before_poll = first.position_reads();
    first.set_position_without_signal(5_500_000);
    wait_for(
        "one-second polling reads position without notification",
        || {
            latest.borrow().position_us == Some(5_500_000)
                && first.position_reads() > reads_before_poll
        },
    );

    first.emit_seeked(9_000_000);
    wait_for(
        "Seeked is accepted even while position polling is active",
        || latest.borrow().position_us == Some(9_000_000),
    );

    first.set_status_and_emit("Paused");
    wait_for("pause change is observed", || {
        latest.borrow().status == PlaybackStatus::Paused
    });
    std::thread::sleep(Duration::from_millis(100));
    pump_context();
    let paused_position = latest.borrow().position_us;
    let reads_while_paused = first.position_reads();
    first.set_position_without_signal(13_000_000);
    std::thread::sleep(Duration::from_millis(1_250));
    pump_context();
    assert_eq!(latest.borrow().position_us, paused_position);
    assert_eq!(first.position_reads(), reads_while_paused);

    first.set_status_and_emit("Playing");
    wait_for("resume is observed and polling restarts", || {
        latest.borrow().status == PlaybackStatus::Playing
            && latest.borrow().position_us == Some(13_000_000)
    });

    // Deliver a real PropertiesChanged invalidation. The service value changes
    // before the signal, so a successful observer update proves GetAll reread.
    first.set_title_and_invalidate("Title after invalidation");
    wait_for(
        "metadata invalidation triggers a full property reread",
        || latest.borrow().title.as_deref() == Some("Title after invalidation"),
    );

    // On the next real Position property read, emit a track switch before the
    // old read reply returns. The old track's response must be discarded.
    first.switch_track_on_next_position_read("Second original title", 77_000_000);
    wait_for("track switch signal is observed", || {
        latest.borrow().title.as_deref() == Some("Second original title")
            && latest.borrow().track_id.as_deref() == Some("/track/two")
    });
    assert_ne!(latest.borrow().position_us, Some(13_000_000));
    assert_ne!(latest.borrow().position_us, Some(77_000_000));

    // A pending read followed by pause must not advance the frozen value.
    first.set_position_action_on_next_read(PositionAction::Pause, 66_000_000);
    let before_pause = latest.borrow().position_us;
    wait_for(
        "pause arriving during a position read rejects that reply",
        || latest.borrow().status == PlaybackStatus::Paused,
    );
    assert_eq!(latest.borrow().position_us, before_pause);

    let reads_before_paused_track_change = first.position_reads();
    first.set_track_and_emit("Paused track title", "/track/three", 18_000_000);
    wait_for(
        "paused track change gets an authoritative position read",
        || {
            let snapshot = latest.borrow();
            snapshot.title.as_deref() == Some("Paused track title")
                && snapshot.track_id.as_deref() == Some("/track/three")
                && snapshot.position_us == Some(18_000_000)
                && first.position_reads() > reads_before_paused_track_change
        },
    );

    // A GetAll already in flight when Seeked arrives must not restore the
    // position it read before the seek.
    let history_start = history.borrow().len();
    first.set_position_action_on_next_read(PositionAction::Seek, 33_000_000);
    first.set_title_and_invalidate("Paused track title after invalidation");
    wait_for("Seeked wins over an older full property reply", || {
        let snapshot = latest.borrow();
        snapshot.title.as_deref() == Some("Paused track title after invalidation")
            && snapshot.position_us == Some(33_000_000)
    });
    let history_after_seek = history.borrow();
    let Some(seek_index) = history_after_seek[history_start..]
        .iter()
        .position(|snapshot| snapshot.position_us == Some(33_000_000))
    else {
        panic!("Seeked position was not emitted in the observer history");
    };
    assert!(
        history_after_seek[history_start + seek_index + 1..]
            .iter()
            .all(|snapshot| snapshot.position_us != Some(18_000_000))
    );
    drop(history_after_seek);

    // Releasing and replacing the well-known name exercises NameOwnerChanged,
    // immediate stale-state clearing, and full state acquisition for new owner.
    first.release_name();
    wait_for("owner loss clears old metadata", || {
        let snapshot = latest.borrow();
        snapshot.availability == Availability::Disconnected
            && snapshot.title.is_none()
            && snapshot.position_us.is_none()
    });
    drop(first);

    let replacement = FakeMpris::start(&bus, "Replacement title", "Paused", 2_000_000);
    wait_for("replacement owner supplies a fresh snapshot", || {
        let snapshot = latest.borrow();
        snapshot.availability == Availability::Connected
            && snapshot.title.as_deref() == Some("Replacement title")
            && snapshot.status == PlaybackStatus::Paused
            && snapshot.position_us == Some(2_000_000)
    });
    assert_eq!(
        observer.snapshot().title.as_deref(),
        Some("Replacement title")
    );

    drop(observer);
    drop(replacement);
}

#[derive(Clone, Copy)]
enum PositionAction {
    None,
    SwitchTrack,
    Pause,
    Seek,
}

struct FakeState {
    title: String,
    status: String,
    position: i64,
    track: &'static str,
    position_reads: usize,
    next_position_action: PositionAction,
    metadata_change_before_reply: Option<String>,
}

impl FakeState {
    fn metadata(&self) -> glib::Variant {
        HashMap::from([
            ("xesam:title".to_owned(), self.title.to_variant()),
            (
                "xesam:artist".to_owned(),
                vec!["Artist One".to_owned(), "Artist Two".to_owned()].to_variant(),
            ),
            (
                "mpris:trackid".to_owned(),
                glib::variant::ObjectPath::try_from(self.track)
                    .unwrap()
                    .to_variant(),
            ),
            ("mpris:length".to_owned(), 120_000_000_i64.to_variant()),
        ])
        .to_variant()
    }
}

struct FakeMpris {
    connection: gio::DBusConnection,
    state: Rc<RefCell<FakeState>>,
    registration: Option<gio::RegistrationId>,
    owns_name: bool,
}

impl FakeMpris {
    fn start(bus: &PrivateBus, title: &str, status: &str, position: i64) -> Self {
        let mut service = Self::prepare(bus, title, status, position);
        service.install_player_object();
        service.request_name();
        service
    }

    fn claim_without_object(bus: &PrivateBus, title: &str, status: &str, position: i64) -> Self {
        let mut service = Self::prepare(bus, title, status, position);
        service.request_name();
        service
    }

    fn prepare(bus: &PrivateBus, title: &str, status: &str, position: i64) -> Self {
        let connection = bus.connect();
        let state = Rc::new(RefCell::new(FakeState {
            title: title.to_owned(),
            status: status.to_owned(),
            position,
            track: "/track/one",
            position_reads: 0,
            next_position_action: PositionAction::None,
            metadata_change_before_reply: None,
        }));
        Self {
            connection,
            state,
            registration: None,
            owns_name: false,
        }
    }

    fn install_player_object(&mut self) {
        if self.registration.is_some() {
            return;
        }
        let node = gio::DBusNodeInfo::for_xml(PLAYER_XML).expect("valid fake Player interface XML");
        let interface = node
            .lookup_interface(PLAYER)
            .expect("Player interface exists in test XML");
        let callback_state = Rc::clone(&self.state);
        let callback_connection = self.connection.clone();
        let registration = self
            .connection
            .register_object(OBJECT_PATH, &interface)
            .property(move |_, _, _, _, name| match name {
                "PlaybackStatus" => callback_state.borrow().status.to_variant(),
                "Metadata" => {
                    let (reply_metadata, changed_metadata) = {
                        let mut state = callback_state.borrow_mut();
                        let reply_metadata = state.metadata();
                        let changed_metadata =
                            state.metadata_change_before_reply.take().map(|title| {
                                state.title = title;
                                state.metadata()
                            });
                        (reply_metadata, changed_metadata)
                    };
                    if let Some(changed) = changed_metadata {
                        emit_properties_changed(
                            &callback_connection,
                            HashMap::from([("Metadata".to_owned(), changed)]),
                            Vec::new(),
                        );
                    }
                    reply_metadata
                }
                "Position" => {
                    let (reply_position, action) = {
                        let mut state = callback_state.borrow_mut();
                        state.position_reads += 1;
                        let reply_position = state.position;
                        let action = std::mem::replace(
                            &mut state.next_position_action,
                            PositionAction::None,
                        );
                        match action {
                            PositionAction::SwitchTrack => {
                                state.title = "Second original title".to_owned();
                                state.track = "/track/two";
                                state.position = 6_000_000;
                            }
                            PositionAction::Pause => {
                                state.status = "Paused".to_owned();
                            }
                            PositionAction::Seek => {
                                state.position = 33_000_000;
                            }
                            PositionAction::None => {}
                        }
                        (reply_position, action)
                    };
                    match action {
                        PositionAction::SwitchTrack => emit_properties_changed(
                            &callback_connection,
                            HashMap::from([(
                                "Metadata".to_owned(),
                                callback_state.borrow().metadata(),
                            )]),
                            Vec::new(),
                        ),
                        PositionAction::Pause => emit_properties_changed(
                            &callback_connection,
                            HashMap::from([("PlaybackStatus".to_owned(), "Paused".to_variant())]),
                            Vec::new(),
                        ),
                        PositionAction::Seek => {
                            callback_connection
                                .emit_signal(
                                    None,
                                    OBJECT_PATH,
                                    PLAYER,
                                    "Seeked",
                                    Some(&(33_000_000_i64,).to_variant()),
                                )
                                .expect("emit private in-flight Seeked signal");
                        }
                        PositionAction::None => {}
                    }
                    reply_position.to_variant()
                }
                other => panic!("unexpected fake MPRIS property: {other}"),
            })
            .build()
            .expect("register fake Player object");
        self.registration = Some(registration);
    }

    fn request_name(&mut self) {
        let response = self
            .connection
            .call_sync(
                Some(DBUS),
                DBUS_PATH,
                DBUS,
                "RequestName",
                Some(&(SERVICE, 0_u32).to_variant()),
                None,
                gio::DBusCallFlags::NONE,
                2_000,
                None::<&gio::Cancellable>,
            )
            .expect("request private test service name");
        assert_eq!(response.get::<(u32,)>().unwrap().0, 1);
        self.owns_name = true;
    }

    fn release_name(&mut self) {
        if !self.owns_name {
            return;
        }
        let response = self
            .connection
            .call_sync(
                Some(DBUS),
                DBUS_PATH,
                DBUS,
                "ReleaseName",
                Some(&(SERVICE,).to_variant()),
                None,
                gio::DBusCallFlags::NONE,
                2_000,
                None::<&gio::Cancellable>,
            )
            .expect("release private test service name");
        assert_eq!(response.get::<(u32,)>().unwrap().0, 1);
        self.owns_name = false;
    }

    fn position_reads(&self) -> usize {
        self.state.borrow().position_reads
    }

    fn set_position_without_signal(&self, position: i64) {
        self.state.borrow_mut().position = position;
    }

    fn emit_seeked(&self, position: i64) {
        self.state.borrow_mut().position = position;
        self.connection
            .emit_signal(
                None,
                OBJECT_PATH,
                PLAYER,
                "Seeked",
                Some(&(position,).to_variant()),
            )
            .expect("emit private Seeked signal");
    }

    fn set_status_and_emit(&self, status: &str) {
        self.state.borrow_mut().status = status.to_owned();
        emit_properties_changed(
            &self.connection,
            HashMap::from([("PlaybackStatus".to_owned(), status.to_variant())]),
            Vec::new(),
        );
    }

    fn set_title_and_invalidate(&self, title: &str) {
        self.state.borrow_mut().title = title.to_owned();
        emit_properties_changed(
            &self.connection,
            HashMap::new(),
            vec!["Metadata".to_owned()],
        );
    }

    fn switch_track_on_next_position_read(&self, title: &str, position: i64) {
        let mut state = self.state.borrow_mut();
        state.title = title.to_owned();
        state.track = "/track/two";
        state.position = position;
        state.next_position_action = PositionAction::SwitchTrack;
    }

    fn set_position_action_on_next_read(&self, action: PositionAction, position: i64) {
        let mut state = self.state.borrow_mut();
        state.position = position;
        state.next_position_action = action;
    }

    fn set_track_and_emit(&self, title: &str, track: &'static str, position: i64) {
        {
            let mut state = self.state.borrow_mut();
            state.title = title.to_owned();
            state.track = track;
            state.position = position;
        }
        emit_properties_changed(
            &self.connection,
            HashMap::from([("Metadata".to_owned(), self.state.borrow().metadata())]),
            Vec::new(),
        );
    }

    fn change_metadata_on_next_read(&self, title: &str) {
        self.state.borrow_mut().metadata_change_before_reply = Some(title.to_owned());
    }
}

impl Drop for FakeMpris {
    fn drop(&mut self) {
        self.release_name();
        if let Some(registration) = self.registration.take() {
            let _ = self.connection.unregister_object(registration);
        }
    }
}

fn emit_properties_changed(
    connection: &gio::DBusConnection,
    changed: HashMap<String, glib::Variant>,
    invalidated: Vec<String>,
) {
    connection
        .emit_signal(
            None,
            OBJECT_PATH,
            PROPERTIES,
            "PropertiesChanged",
            Some(&(PLAYER, changed, invalidated).to_variant()),
        )
        .expect("emit private PropertiesChanged signal");
}

struct PrivateBus {
    child: Child,
    address: String,
}

impl PrivateBus {
    fn start() -> Self {
        let mut child = Command::new("dbus-daemon")
            .args([
                "--session",
                "--nofork",
                "--print-address=1",
                "--print-pid=1",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("dbus-daemon is required for isolated MPRIS protocol tests");
        let stdout = child.stdout.take().expect("private bus stdout is piped");
        let mut reader = BufReader::new(stdout);
        let mut address = String::new();
        let mut pid = String::new();
        reader
            .read_line(&mut address)
            .expect("read private D-Bus address");
        reader.read_line(&mut pid).expect("read private D-Bus pid");
        assert!(
            !address.trim().is_empty(),
            "private D-Bus address was empty"
        );
        assert!(
            pid.trim().parse::<u32>().is_ok(),
            "private D-Bus pid was invalid"
        );
        drop(reader);
        Self {
            child,
            address: address.trim().to_owned(),
        }
    }

    fn connect(&self) -> gio::DBusConnection {
        let connection = gio::DBusConnection::for_address_sync(
            &self.address,
            gio::DBusConnectionFlags::AUTHENTICATION_CLIENT
                | gio::DBusConnectionFlags::MESSAGE_BUS_CONNECTION,
            None,
            None::<&gio::Cancellable>,
        )
        .expect("connect to private D-Bus daemon");
        connection.set_exit_on_close(false);
        connection
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn wait_for(description: &str, mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !predicate() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {description}"
        );
        pump_context();
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn pump_context() {
    let context = glib::MainContext::default();
    while context.pending() {
        context.iteration(false);
    }
}
