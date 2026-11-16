//! Read-only observation of Spotify's MPRIS player on the session bus.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::sync::mpsc;
use std::time::Duration;

use gtk::gio;
use gtk::glib;
use gtk::glib::variant::ToVariant;
use gtk::prelude::*;

const SERVICE: &str = "org.mpris.MediaPlayer2.spotify";
const OBJECT_PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
const DBUS: &str = "org.freedesktop.DBus";
const DBUS_PATH: &str = "/org/freedesktop/DBus";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const CALL_TIMEOUT_MS: i32 = 3_000;
const POSITION_INTERVAL: Duration = Duration::from_secs(1);
const PROPERTY_RETRY_INTERVAL: Duration = Duration::from_secs(1);
const RECONNECT_INTERVAL: Duration = Duration::from_secs(3);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Availability {
    #[default]
    Connecting,
    Disconnected,
    Connected,
    Error(String),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
    #[default]
    Unknown,
}

/// A display-ready, immutable copy of the latest accepted Spotify state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PlaybackSnapshot {
    pub availability: Availability,
    pub title: Option<String>,
    pub artists: Vec<String>,
    pub status: PlaybackStatus,
    pub position_us: Option<u64>,
    pub duration_us: Option<u64>,
    pub track_id: Option<String>,
}

/// Observes Spotify without launching it or sending playback-control methods.
///
/// The callback runs on the GLib main context. Dropping the observer stops its
/// timers, removes signal subscriptions and cancels an in-flight position read.
pub struct PlaybackObserver {
    inner: Rc<RefCell<ObserverState>>,
}

impl PlaybackObserver {
    /// Connect to the session bus asynchronously and begin observing Spotify.
    pub fn new(on_change: impl Fn(PlaybackSnapshot) + 'static) -> Self {
        let observer = Self::with_initial_snapshot(on_change);
        notify(&observer.inner);
        connect_session_bus(Rc::downgrade(&observer.inner));
        observer
    }

    /// Begin observing over an existing connection. This is also useful to
    /// embed the observer in an application that already owns its bus setup.
    pub fn from_connection(
        connection: gio::DBusConnection,
        on_change: impl Fn(PlaybackSnapshot) + 'static,
    ) -> Self {
        let observer = Self::with_initial_snapshot(on_change);
        attach_connection(&observer.inner, connection);
        observer
    }

    fn with_initial_snapshot(on_change: impl Fn(PlaybackSnapshot) + 'static) -> Self {
        Self {
            inner: Rc::new(RefCell::new(ObserverState {
                snapshot: PlaybackSnapshot::default(),
                on_change: Rc::new(on_change),
                connection: None,
                bus_subscription: None,
                player_subscriptions: Vec::new(),
                owner: None,
                owner_generation: 0,
                owner_event_revision: 0,
                metadata_revision: 0,
                status_revision: 0,
                track_generation: 0,
                transport_revision: 0,
                position_revision: 0,
                position_request: 0,
                position_in_flight: None,
                poll_source: None,
                connection_monitor_source: None,
                property_retry_source: None,
                reconnect_source: None,
                stopped: false,
                connect_attempt: 0,
            })),
        }
    }

    pub fn snapshot(&self) -> PlaybackSnapshot {
        self.inner.borrow().snapshot.clone()
    }
}

impl Drop for PlaybackObserver {
    fn drop(&mut self) {
        stop(&self.inner);
    }
}

struct ObserverState {
    snapshot: PlaybackSnapshot,
    on_change: Rc<dyn Fn(PlaybackSnapshot)>,
    connection: Option<gio::DBusConnection>,
    bus_subscription: Option<gio::SignalSubscription>,
    player_subscriptions: Vec<gio::SignalSubscription>,
    owner: Option<String>,
    owner_generation: u64,
    owner_event_revision: u64,
    metadata_revision: u64,
    status_revision: u64,
    track_generation: u64,
    transport_revision: u64,
    position_revision: u64,
    position_request: u64,
    position_in_flight: Option<gio::Cancellable>,
    poll_source: Option<glib::SourceId>,
    connection_monitor_source: Option<glib::SourceId>,
    property_retry_source: Option<glib::SourceId>,
    reconnect_source: Option<glib::SourceId>,
    stopped: bool,
    connect_attempt: u64,
}

#[derive(Default)]
struct TrackMetadata {
    title: Option<String>,
    artists: Vec<String>,
    track_id: Option<String>,
    duration_us: Option<u64>,
}

fn notify(inner: &Rc<RefCell<ObserverState>>) {
    let (callback, snapshot) = {
        let state = inner.borrow();
        if state.stopped {
            return;
        }
        (Rc::clone(&state.on_change), state.snapshot.clone())
    };
    callback(snapshot);
}

fn connect_session_bus(inner: Weak<RefCell<ObserverState>>) {
    let Some(inner_rc) = inner.upgrade() else {
        return;
    };
    let attempt = {
        let mut state = inner_rc.borrow_mut();
        if state.stopped {
            return;
        }
        state.connect_attempt = state.connect_attempt.wrapping_add(1);
        state.connect_attempt
    };
    set_availability(&inner_rc, Availability::Connecting);

    let address = match gio::dbus_address_get_for_bus_sync(
        gio::BusType::Session,
        None::<&gio::Cancellable>,
    ) {
        Ok(address) => address,
        Err(error) => {
            set_availability(&inner_rc, Availability::Error(error.to_string()));
            schedule_reconnect(&inner_rc);
            return;
        }
    };

    gio::DBusConnection::for_address(
        address.as_str(),
        gio::DBusConnectionFlags::AUTHENTICATION_CLIENT
            | gio::DBusConnectionFlags::MESSAGE_BUS_CONNECTION,
        None,
        None::<&gio::Cancellable>,
        move |result: Result<gio::DBusConnection, glib::Error>| {
            let Some(inner_rc) = inner.upgrade() else {
                return;
            };
            let current = {
                let state = inner_rc.borrow();
                !state.stopped && state.connect_attempt == attempt
            };
            if !current {
                return;
            }
            match result {
                Ok(connection) => attach_connection(&inner_rc, connection),
                Err(error) => {
                    set_availability(&inner_rc, Availability::Error(error.to_string()));
                    schedule_reconnect(&inner_rc);
                }
            }
        },
    );
}

fn attach_connection(inner: &Rc<RefCell<ObserverState>>, connection: gio::DBusConnection) {
    connection.set_exit_on_close(false);
    {
        let mut state = inner.borrow_mut();
        if state.stopped {
            return;
        }
        state.connection = Some(connection.clone());
        state.owner_event_revision = state.owner_event_revision.wrapping_add(1);
    }
    set_availability(inner, Availability::Disconnected);

    let weak = Rc::downgrade(inner);
    let bus_subscription = connection.subscribe_to_signal(
        Some(DBUS),
        Some(DBUS),
        Some("NameOwnerChanged"),
        Some(DBUS_PATH),
        Some(SERVICE),
        gio::DBusSignalFlags::NONE,
        move |signal| {
            let Some(inner) = weak.upgrade() else {
                return;
            };
            let Some((name, _old_owner, new_owner)) =
                signal.parameters.get::<(String, String, String)>()
            else {
                return;
            };
            if name != SERVICE {
                return;
            }
            {
                let mut state = inner.borrow_mut();
                state.owner_event_revision = state.owner_event_revision.wrapping_add(1);
            }
            handle_owner_change(&inner, (!new_owner.is_empty()).then_some(new_owner));
        },
    );
    inner.borrow_mut().bus_subscription = Some(bus_subscription);

    monitor_connection_close(inner, &connection);
    let revision = inner.borrow().owner_event_revision;
    connection.call(
        Some(DBUS),
        DBUS_PATH,
        DBUS,
        "GetNameOwner",
        Some(&(SERVICE,).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        CALL_TIMEOUT_MS,
        None::<&gio::Cancellable>,
        {
            let weak = Rc::downgrade(inner);
            move |result| {
                let Some(inner) = weak.upgrade() else {
                    return;
                };
                if inner.borrow().owner_event_revision != revision {
                    return;
                }
                match result.and_then(|value| {
                    value
                        .get::<(String,)>()
                        .map(|(owner,)| owner)
                        .ok_or_else(|| {
                            glib::Error::new(
                                gio::IOErrorEnum::InvalidData,
                                "Invalid GetNameOwner response",
                            )
                        })
                }) {
                    Ok(owner) => handle_owner_change(&inner, Some(owner)),
                    Err(error) if is_no_owner_error(&error) => handle_owner_change(&inner, None),
                    Err(error) => set_availability(&inner, Availability::Error(error.to_string())),
                }
            }
        },
    );
}

fn monitor_connection_close(inner: &Rc<RefCell<ObserverState>>, connection: &gio::DBusConnection) {
    let (closed_sender, closed_receiver) = mpsc::channel::<String>();
    connection.connect_closed(move |_, remote_peer_vanished, error| {
        let detail = error.map(|error| error.to_string()).unwrap_or_else(|| {
            if remote_peer_vanished {
                "The session bus connection closed.".to_owned()
            } else {
                "The session bus connection was closed.".to_owned()
            }
        });
        let _ = closed_sender.send(detail);
    });

    let weak = Rc::downgrade(inner);
    let observed_connection = connection.clone();
    let source = glib::timeout_add_local(Duration::from_millis(100), move || {
        let Ok(detail) = closed_receiver.try_recv() else {
            return if weak.upgrade().is_some() {
                glib::ControlFlow::Continue
            } else {
                glib::ControlFlow::Break
            };
        };
        let Some(inner) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        let is_current = inner
            .borrow()
            .connection
            .as_ref()
            .is_some_and(|current| current == &observed_connection);
        if !is_current {
            return glib::ControlFlow::Break;
        }
        inner.borrow_mut().connection_monitor_source = None;
        detach_connection(&inner);
        set_availability(&inner, Availability::Error(detail));
        schedule_reconnect(&inner);
        glib::ControlFlow::Break
    });
    inner.borrow_mut().connection_monitor_source = Some(source);
}

fn is_no_owner_error(error: &glib::Error) -> bool {
    error.matches(gio::DBusError::NameHasNoOwner) || error.matches(gio::IOErrorEnum::NotFound)
}

fn handle_owner_change(inner: &Rc<RefCell<ObserverState>>, owner: Option<String>) {
    let (changed, generation) = {
        let mut state = inner.borrow_mut();
        if state.stopped || state.owner == owner {
            return;
        }
        invalidate_owner_work(&mut state);
        state.owner_generation = state.owner_generation.wrapping_add(1);
        state.owner = owner.clone();
        state.metadata_revision = state.metadata_revision.wrapping_add(1);
        state.status_revision = state.status_revision.wrapping_add(1);
        state.track_generation = state.track_generation.wrapping_add(1);
        state.transport_revision = state.transport_revision.wrapping_add(1);
        state.position_revision = state.position_revision.wrapping_add(1);
        state.snapshot = empty_snapshot(if owner.is_some() {
            Availability::Connected
        } else {
            Availability::Disconnected
        });
        (true, state.owner_generation)
    };
    if changed {
        notify(inner);
    }

    let Some(owner) = owner else {
        return;
    };
    let Some(connection) = inner.borrow().connection.clone() else {
        return;
    };

    let weak = Rc::downgrade(inner);
    let properties_subscription = connection.subscribe_to_signal(
        Some(&owner),
        Some(PROPERTIES),
        Some("PropertiesChanged"),
        Some(OBJECT_PATH),
        Some(PLAYER),
        gio::DBusSignalFlags::NONE,
        move |signal| {
            let Some(inner) = weak.upgrade() else {
                return;
            };
            on_properties_changed(&inner, generation, signal.parameters);
        },
    );
    let weak = Rc::downgrade(inner);
    let seeked_subscription = connection.subscribe_to_signal(
        Some(&owner),
        Some(PLAYER),
        Some("Seeked"),
        Some(OBJECT_PATH),
        None,
        gio::DBusSignalFlags::NONE,
        move |signal| {
            let Some(inner) = weak.upgrade() else {
                return;
            };
            on_seeked(&inner, generation, signal.parameters);
        },
    );
    inner
        .borrow_mut()
        .player_subscriptions
        .extend([properties_subscription, seeked_subscription]);
    request_properties(inner, owner, generation);
}

fn invalidate_owner_work(state: &mut ObserverState) {
    state.player_subscriptions.clear();
    if let Some(source) = state.poll_source.take() {
        source.remove();
    }
    if let Some(source) = state.property_retry_source.take() {
        source.remove();
    }
    cancel_position_read(state);
}

fn request_properties(inner: &Rc<RefCell<ObserverState>>, owner: String, generation: u64) {
    let (Some(connection), metadata_revision, status_revision, position_revision, track_generation) =
        ({
            let state = inner.borrow();
            if state.stopped
                || state.owner_generation != generation
                || state.owner.as_deref() != Some(&owner)
            {
                return;
            }
            (
                state.connection.clone(),
                state.metadata_revision,
                state.status_revision,
                state.position_revision,
                state.track_generation,
            )
        })
    else {
        return;
    };
    let callback_owner = owner.clone();

    connection.call(
        Some(&owner),
        OBJECT_PATH,
        PROPERTIES,
        "GetAll",
        Some(&(PLAYER,).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        CALL_TIMEOUT_MS,
        None::<&gio::Cancellable>,
        {
            let weak = Rc::downgrade(inner);
            move |result| {
                let Some(inner) = weak.upgrade() else {
                    return;
                };
                let (current, metadata_current, status_current, position_current) = {
                    let state = inner.borrow();
                    let current = !state.stopped
                        && state.owner_generation == generation
                        && state.owner.as_deref() == Some(&callback_owner);
                    (
                        current,
                        state.metadata_revision == metadata_revision
                            && state.track_generation == track_generation,
                        state.status_revision == status_revision,
                        state.position_revision == position_revision
                            && state.status_revision == status_revision
                            && state.track_generation == track_generation,
                    )
                };
                if !current {
                    return;
                }
                match result {
                    Ok(value) => {
                        if let Some((properties,)) =
                            value.get::<(HashMap<String, glib::Variant>,)>()
                        {
                            clear_property_retry(&inner);
                            apply_full_properties(
                                &inner,
                                &properties,
                                metadata_current,
                                status_current,
                                position_current,
                            );
                            let incomplete = {
                                let state = inner.borrow();
                                (!metadata_current
                                    && state.snapshot.title.is_none()
                                    && state.snapshot.track_id.is_none())
                                    || (!status_current
                                        && state.snapshot.status == PlaybackStatus::Unknown)
                                    || (!position_current
                                        && state.snapshot.position_us.is_none()
                                        && matches!(
                                            state.snapshot.status,
                                            PlaybackStatus::Playing | PlaybackStatus::Paused
                                        ))
                            };
                            if incomplete {
                                request_properties(&inner, callback_owner, generation);
                            }
                        } else {
                            set_availability(
                                &inner,
                                Availability::Error(
                                    "Spotify returned malformed properties.".to_owned(),
                                ),
                            );
                            schedule_property_retry(&inner, callback_owner, generation);
                        }
                    }
                    Err(error) => {
                        set_availability(&inner, Availability::Error(error.to_string()));
                        schedule_property_retry(&inner, callback_owner, generation);
                    }
                }
            }
        },
    );
}

fn on_properties_changed(
    inner: &Rc<RefCell<ObserverState>>,
    generation: u64,
    parameters: &glib::Variant,
) {
    let Some((interface, changed, invalidated)) =
        parameters.get::<(String, HashMap<String, glib::Variant>, Vec<String>)>()
    else {
        return;
    };
    if interface != PLAYER {
        return;
    }

    let relevant_change = ["Metadata", "PlaybackStatus", "Position"]
        .iter()
        .any(|key| changed.contains_key(*key) || invalidated.iter().any(|entry| entry == key));
    if !relevant_change {
        return;
    }

    let has_relevant_invalidation = invalidated
        .iter()
        .any(|key| matches!(key.as_str(), "Metadata" | "PlaybackStatus" | "Position"));
    let mut track_changed = false;
    let mut needs_refresh = has_relevant_invalidation;
    {
        let mut state = inner.borrow_mut();
        if state.stopped || state.owner_generation != generation || state.owner.is_none() {
            return;
        }
        if changed.contains_key("Metadata") || invalidated.iter().any(|key| key == "Metadata") {
            state.metadata_revision = state.metadata_revision.wrapping_add(1);
        }
        if changed.contains_key("PlaybackStatus")
            || invalidated.iter().any(|key| key == "PlaybackStatus")
        {
            state.status_revision = state.status_revision.wrapping_add(1);
        }
        for key in &invalidated {
            match key.as_str() {
                "Metadata" => {
                    set_metadata(&mut state, TrackMetadata::default(), true);
                    state.snapshot.position_us = None;
                }
                "PlaybackStatus" => set_status(&mut state, PlaybackStatus::Unknown),
                "Position" => {
                    state.position_revision = state.position_revision.wrapping_add(1);
                    cancel_position_read(&mut state);
                    state.snapshot.position_us = None;
                }
                _ => {}
            }
        }
        if let Some(metadata) = changed.get("Metadata") {
            track_changed = set_metadata(&mut state, parse_metadata(metadata), true);
        }
        if let Some(status) = changed.get("PlaybackStatus") {
            set_status(&mut state, parse_status(status));
        }
        if let Some(position) = changed.get("Position").and_then(parse_position) {
            state.position_revision = state.position_revision.wrapping_add(1);
            cancel_position_read(&mut state);
            state.snapshot.position_us = Some(clamp_position(position, state.snapshot.duration_us));
        }
        if matches!(state.snapshot.availability, Availability::Error(_)) {
            state.snapshot.availability = Availability::Connected;
        }
        needs_refresh |= state.snapshot.status == PlaybackStatus::Unknown;
        let poll_now = update_polling(inner, &mut state);
        drop(state);
        if poll_now {
            poll_position(inner);
        }
    }
    notify(inner);

    if track_changed {
        request_position(inner, true);
    }

    if needs_refresh && let Some(owner) = inner.borrow().owner.clone() {
        request_properties(inner, owner, generation);
    }
}

fn on_seeked(inner: &Rc<RefCell<ObserverState>>, generation: u64, parameters: &glib::Variant) {
    let Some((position,)) = parameters.get::<(i64,)>() else {
        return;
    };
    let Some(position) = nonnegative(position) else {
        return;
    };
    {
        let mut state = inner.borrow_mut();
        if state.stopped || state.owner_generation != generation || state.owner.is_none() {
            return;
        }
        state.position_revision = state.position_revision.wrapping_add(1);
        cancel_position_read(&mut state);
        state.snapshot.position_us = Some(clamp_position(position, state.snapshot.duration_us));
    }
    notify(inner);
    if let Some(owner) = inner.borrow().owner.clone() {
        request_properties(inner, owner, generation);
    }
}

fn apply_full_properties(
    inner: &Rc<RefCell<ObserverState>>,
    properties: &HashMap<String, glib::Variant>,
    accept_metadata: bool,
    accept_status: bool,
    accept_position: bool,
) {
    let (track_changed, has_position) = {
        let mut state = inner.borrow_mut();
        let track_changed = accept_metadata
            && set_metadata(
                &mut state,
                properties
                    .get("Metadata")
                    .map(parse_metadata)
                    .unwrap_or_default(),
                false,
            );
        if accept_status {
            set_status(
                &mut state,
                properties
                    .get("PlaybackStatus")
                    .map(parse_status)
                    .unwrap_or(PlaybackStatus::Unknown),
            );
        }
        let position = accept_position
            .then(|| properties.get("Position").and_then(parse_position))
            .flatten();
        if let Some(position) = position {
            state.position_revision = state.position_revision.wrapping_add(1);
            cancel_position_read(&mut state);
            state.snapshot.position_us = Some(clamp_position(position, state.snapshot.duration_us));
        }
        if matches!(state.snapshot.availability, Availability::Error(_)) {
            state.snapshot.availability = Availability::Connected;
        }
        let poll_now = update_polling(inner, &mut state);
        drop(state);
        if poll_now {
            poll_position(inner);
        }
        (track_changed, position.is_some())
    };
    notify(inner);
    if track_changed && !has_position {
        request_position(inner, true);
    }
}

fn set_metadata(
    state: &mut ObserverState,
    metadata: TrackMetadata,
    force_track_epoch: bool,
) -> bool {
    let track_changed = force_track_epoch
        || state.snapshot.track_id != metadata.track_id
        || state.snapshot.title != metadata.title
        || state.snapshot.artists != metadata.artists
        || state.snapshot.duration_us != metadata.duration_us;
    if track_changed {
        state.track_generation = state.track_generation.wrapping_add(1);
        state.position_revision = state.position_revision.wrapping_add(1);
        cancel_position_read(state);
        state.snapshot.position_us = None;
    }
    state.snapshot.track_id = metadata.track_id;
    state.snapshot.title = metadata.title;
    state.snapshot.artists = metadata.artists;
    state.snapshot.duration_us = metadata.duration_us;
    if let (Some(position), Some(duration)) =
        (state.snapshot.position_us, state.snapshot.duration_us)
    {
        state.snapshot.position_us = Some(position.min(duration));
    }
    track_changed
}

fn set_status(state: &mut ObserverState, status: PlaybackStatus) {
    if state.snapshot.status != status {
        state.transport_revision = state.transport_revision.wrapping_add(1);
        cancel_position_read(state);
        state.snapshot.status = status;
    }
}

fn update_polling(inner: &Rc<RefCell<ObserverState>>, state: &mut ObserverState) -> bool {
    let should_poll = state.snapshot.availability == Availability::Connected
        && state.snapshot.status == PlaybackStatus::Playing
        && state.owner.is_some();
    if should_poll && state.poll_source.is_none() {
        let weak = Rc::downgrade(inner);
        state.poll_source = Some(glib::timeout_add_local(POSITION_INTERVAL, move || {
            let Some(inner) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            poll_position(&inner);
            glib::ControlFlow::Continue
        }));
        true
    } else if !should_poll && let Some(source) = state.poll_source.take() {
        source.remove();
        false
    } else {
        false
    }
}

fn poll_position(inner: &Rc<RefCell<ObserverState>>) {
    request_position(inner, false);
}

fn request_position(inner: &Rc<RefCell<ObserverState>>, allow_paused: bool) {
    let (
        connection,
        owner,
        generation,
        track_generation,
        transport_revision,
        position_revision,
        status,
        track_id,
        cancellable,
        request,
    ) = {
        let mut state = inner.borrow_mut();
        if state.stopped
            || state.position_in_flight.is_some()
            || state.snapshot.availability != Availability::Connected
            || (state.snapshot.status != PlaybackStatus::Playing
                && !(allow_paused && state.snapshot.status == PlaybackStatus::Paused))
        {
            return;
        }
        let (Some(connection), Some(owner)) = (state.connection.clone(), state.owner.clone())
        else {
            return;
        };
        state.position_request = state.position_request.wrapping_add(1);
        let request = state.position_request;
        let cancellable = gio::Cancellable::new();
        state.position_in_flight = Some(cancellable.clone());
        (
            connection,
            owner,
            state.owner_generation,
            state.track_generation,
            state.transport_revision,
            state.position_revision,
            state.snapshot.status,
            state.snapshot.track_id.clone(),
            cancellable,
            request,
        )
    };

    let callback_owner = owner.clone();
    connection.call(
        Some(&owner),
        OBJECT_PATH,
        PROPERTIES,
        "Get",
        Some(&(PLAYER, "Position").to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        CALL_TIMEOUT_MS,
        Some(&cancellable),
        {
            let weak = Rc::downgrade(inner);
            move |result| {
                let Some(inner) = weak.upgrade() else {
                    return;
                };
                let mut accepted = false;
                {
                    let mut state = inner.borrow_mut();
                    let request_is_current = state.position_request == request;
                    if request_is_current {
                        state.position_in_flight = None;
                    }
                    if !state.stopped
                        && request_is_current
                        && state.owner_generation == generation
                        && state.track_generation == track_generation
                        && state.transport_revision == transport_revision
                        && state.position_revision == position_revision
                        && state.owner.as_deref() == Some(&callback_owner)
                        && state.snapshot.track_id == track_id
                        && state.snapshot.status == status
                        && let Ok(value) = result
                        && let Some((wrapped,)) = value.get::<(glib::Variant,)>()
                        && let Some(position) = parse_position(&wrapped)
                    {
                        state.snapshot.position_us =
                            Some(clamp_position(position, state.snapshot.duration_us));
                        state.position_revision = state.position_revision.wrapping_add(1);
                        accepted = true;
                    }
                }
                if accepted {
                    notify(&inner);
                }
            }
        },
    );
}

fn cancel_position_read(state: &mut ObserverState) {
    if let Some(cancellable) = state.position_in_flight.take() {
        state.position_request = state.position_request.wrapping_add(1);
        cancellable.cancel();
    }
}

fn set_availability(inner: &Rc<RefCell<ObserverState>>, availability: Availability) {
    let changed = {
        let mut state = inner.borrow_mut();
        if state.stopped || state.snapshot.availability == availability {
            false
        } else {
            state.snapshot.availability = availability;
            true
        }
    };
    if changed {
        notify(inner);
    }
}

fn empty_snapshot(availability: Availability) -> PlaybackSnapshot {
    PlaybackSnapshot {
        availability,
        ..PlaybackSnapshot::default()
    }
}

fn detach_connection(inner: &Rc<RefCell<ObserverState>>) {
    let mut state = inner.borrow_mut();
    state.owner_generation = state.owner_generation.wrapping_add(1);
    state.owner = None;
    invalidate_owner_work(&mut state);
    if let Some(source) = state.connection_monitor_source.take() {
        source.remove();
    }
    state.bus_subscription = None;
    state.connection = None;
    state.snapshot = empty_snapshot(Availability::Disconnected);
}

fn schedule_reconnect(inner: &Rc<RefCell<ObserverState>>) {
    let mut state = inner.borrow_mut();
    if state.stopped || state.reconnect_source.is_some() {
        return;
    }
    let weak = Rc::downgrade(inner);
    state.reconnect_source = Some(glib::timeout_add_local_once(
        RECONNECT_INTERVAL,
        move || {
            if let Some(inner) = weak.upgrade() {
                inner.borrow_mut().reconnect_source = None;
                connect_session_bus(Rc::downgrade(&inner));
            }
        },
    ));
}

fn schedule_property_retry(inner: &Rc<RefCell<ObserverState>>, owner: String, generation: u64) {
    let mut state = inner.borrow_mut();
    if state.stopped
        || state.property_retry_source.is_some()
        || state.owner_generation != generation
        || state.owner.as_deref() != Some(&owner)
    {
        return;
    }
    let weak = Rc::downgrade(inner);
    state.property_retry_source = Some(glib::timeout_add_local_once(
        PROPERTY_RETRY_INTERVAL,
        move || {
            if let Some(inner) = weak.upgrade() {
                let still_current = {
                    let mut state = inner.borrow_mut();
                    state.property_retry_source = None;
                    !state.stopped
                        && state.owner_generation == generation
                        && state.owner.as_deref() == Some(&owner)
                };
                if still_current {
                    request_properties(&inner, owner, generation);
                }
            }
        },
    ));
}

fn clear_property_retry(inner: &Rc<RefCell<ObserverState>>) {
    let mut state = inner.borrow_mut();
    if let Some(source) = state.property_retry_source.take() {
        source.remove();
    }
}

fn stop(inner: &Rc<RefCell<ObserverState>>) {
    let mut state = inner.borrow_mut();
    if state.stopped {
        return;
    }
    state.stopped = true;
    invalidate_owner_work(&mut state);
    if let Some(source) = state.connection_monitor_source.take() {
        source.remove();
    }
    state.bus_subscription = None;
    state.connection = None;
    if let Some(source) = state.reconnect_source.take() {
        source.remove();
    }
}

fn parse_metadata(value: &glib::Variant) -> TrackMetadata {
    let Some(metadata) = value.get::<HashMap<String, glib::Variant>>() else {
        return TrackMetadata::default();
    };
    let title = metadata
        .get("xesam:title")
        .and_then(|value| value.get::<String>());
    let artists = metadata
        .get("xesam:artist")
        .and_then(|value| value.get::<Vec<String>>())
        .unwrap_or_default();
    let track_id = metadata
        .get("mpris:trackid")
        .and_then(|value| value.get::<glib::variant::ObjectPath>())
        .map(|path| path.to_string());
    let duration_us = metadata
        .get("mpris:length")
        .and_then(|value| value.get::<i64>())
        .and_then(nonnegative);
    TrackMetadata {
        title,
        artists,
        track_id,
        duration_us,
    }
}

fn parse_status(value: &glib::Variant) -> PlaybackStatus {
    match value.get::<String>().as_deref() {
        Some("Playing") => PlaybackStatus::Playing,
        Some("Paused") => PlaybackStatus::Paused,
        Some("Stopped") => PlaybackStatus::Stopped,
        _ => PlaybackStatus::Unknown,
    }
}

fn parse_position(value: &glib::Variant) -> Option<u64> {
    value.get::<i64>().and_then(nonnegative)
}

fn nonnegative(value: i64) -> Option<u64> {
    u64::try_from(value).ok()
}

fn clamp_position(position: u64, duration: Option<u64>) -> u64 {
    duration.map_or(position, |duration| position.min(duration))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk::glib::variant::ToVariant;

    #[test]
    fn metadata_accepts_all_artists_and_optional_mpris_fields() {
        let metadata = HashMap::from([
            ("xesam:title".to_owned(), "Original title".to_variant()),
            (
                "xesam:artist".to_owned(),
                vec!["First artist".to_owned(), "Second artist".to_owned()].to_variant(),
            ),
            (
                "mpris:trackid".to_owned(),
                glib::variant::ObjectPath::try_from("/track/one")
                    .unwrap()
                    .to_variant(),
            ),
            ("mpris:length".to_owned(), 90_000_000_i64.to_variant()),
        ])
        .to_variant();

        let parsed = parse_metadata(&metadata);
        assert_eq!(parsed.title.as_deref(), Some("Original title"));
        assert_eq!(parsed.artists, ["First artist", "Second artist"]);
        assert_eq!(parsed.track_id.as_deref(), Some("/track/one"));
        assert_eq!(parsed.duration_us, Some(90_000_000));
    }

    #[test]
    fn malformed_and_missing_metadata_are_safe_and_optional() {
        let malformed = HashMap::from([
            ("xesam:title".to_owned(), 12_i32.to_variant()),
            ("xesam:artist".to_owned(), "not-an-array".to_variant()),
            ("mpris:length".to_owned(), (-1_i64).to_variant()),
        ])
        .to_variant();

        let parsed = parse_metadata(&malformed);
        assert_eq!(parsed.title, None);
        assert!(parsed.artists.is_empty());
        assert_eq!(parsed.track_id, None);
        assert_eq!(parsed.duration_us, None);
        assert_eq!(parse_metadata(&().to_variant()).title, None);
    }

    #[test]
    fn status_and_position_decoders_reject_wrong_types_and_negative_values() {
        assert_eq!(
            parse_status(&"Playing".to_variant()),
            PlaybackStatus::Playing
        );
        assert_eq!(
            parse_status(&"Unexpected".to_variant()),
            PlaybackStatus::Unknown
        );
        assert_eq!(parse_status(&1_i32.to_variant()), PlaybackStatus::Unknown);
        assert_eq!(parse_position(&(-1_i64).to_variant()), None);
        assert_eq!(parse_position(&7_i64.to_variant()), Some(7));
        assert_eq!(parse_position(&"7".to_variant()), None);
        assert_eq!(clamp_position(11, Some(10)), 10);
    }
}
