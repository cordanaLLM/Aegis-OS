// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Real `zbus` `StatusNotifierWatcher` adapter for the P05 shell (M29 E29-3).
//!
//! Provides [`ZbusWatcher`], which implements [`StatusNotifierWatcher`] over a
//! [`zbus::Connection`]. Connects to `org.kde.StatusNotifierWatcher` on the
//! session bus, claims [`HOST_SERVICE`] before registering as a host, and
//! queries registered items and host status under explicit deadlines (HISS-02).
//!
//! Tracks late-appearing watcher instances using [`WatcherRegistration`].
//!
//! ### Specification Reference
//!
//! Verified against upstream KDE `StatusNotifierWatcher` interfaces:
//! - KDE Frameworks `kstatusnotifieritem`:
//!   `https://invent.kde.org/frameworks/kstatusnotifieritem/-/blob/1ba33a9809ee2b2b574009edb6d77feb627eecb6/src/org.kde.StatusNotifierWatcher.xml`
//! - KDE Plasma Workspace `plasma-workspace`:
//!   `https://invent.kde.org/plasma/plasma-workspace/-/blob/3fd412558d9107e7db746a8a9ce450c7d3c41502/statusnotifierwatcher/statusnotifierwatcher.cpp`
//! - Freedesktop `StatusNotifierItem` / `StatusNotifierWatcher` specification:
//!   `https://www.freedesktop.org/wiki/Specifications/StatusNotifierItem/StatusNotifierWatcher/`
//!
//! ### Interpretation of "watcher lists it" (REQ-P05-05)
//!
//! In the `StatusNotifierItem` (SNI) specification, `org.kde.StatusNotifierWatcher`
//! exposes an array property for registered notification items
//! (`RegisteredStatusNotifierItems: as`), but does not expose an array property
//! for registered hosts. Instead, the interface provides a boolean property
//! `IsStatusNotifierHostRegistered: b` indicating whether at least one host is
//! registered.
//!
//! Therefore, "watcher lists it" for a host registration is read as the watcher
//! reporting `IsStatusNotifierHostRegistered == true` after the
//! `RegisterStatusNotifierHost` call.
//!
//! If downstream consumers or live-session tests require an item registration
//! (`RegisterStatusNotifierItem`) instead of host registration to appear in
//! `RegisteredStatusNotifierItems`, that distinction is recorded as an open question.

use std::time::Duration;

use aegis_forum_shell::platform::{HOST_SERVICE, PlatformError, StatusNotifierWatcher};
use futures_lite::StreamExt;
use zbus::names::WellKnownName;

/// The well-known D-Bus service name of the `StatusNotifierWatcher` daemon.
pub const WATCHER_SERVICE: &str = "org.kde.StatusNotifierWatcher";

/// The object path of the `StatusNotifierWatcher` service.
pub const WATCHER_PATH: &str = "/StatusNotifierWatcher";

/// The D-Bus interface name of the `StatusNotifierWatcher` service.
pub const WATCHER_INTERFACE: &str = "org.kde.StatusNotifierWatcher";

/// Default deadline duration for watcher D-Bus roundtrips.
pub const WATCHER_DEADLINE: Duration = Duration::from_secs(2);

/// Identifies a specific D-Bus operation on the watcher interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Call {
    Subscribe,
    RequestName,
    RegisterHost,
    IsHostRegistered,
    RegisteredItems,
}

impl Call {
    const fn name(self) -> &'static str {
        match self {
            Self::Subscribe => "NameOwnerChanged",
            Self::RequestName => "request_name",
            Self::RegisterHost => "RegisterStatusNotifierHost",
            Self::IsHostRegistered => "IsStatusNotifierHostRegistered",
            Self::RegisteredItems => "RegisteredStatusNotifierItems",
        }
    }

    const fn deadline_text(self) -> &'static str {
        match self {
            Self::Subscribe => "NameOwnerChanged deadline",
            Self::RequestName => "request_name deadline",
            Self::RegisterHost => "RegisterStatusNotifierHost deadline",
            Self::IsHostRegistered => "IsStatusNotifierHostRegistered deadline",
            Self::RegisteredItems => "RegisteredStatusNotifierItems deadline",
        }
    }
}

/// Errors arising during watcher D-Bus calls before mapping to [`PlatformError`].
#[derive(Debug)]
enum WatcherCallError {
    Deadline(Call),
    Bus(zbus::Error),
}

enum SignalEvent {
    Signal(zbus::fdo::NameOwnerChanged),
    Closed,
    Timeout,
}

#[zbus::proxy(
    interface = "org.kde.StatusNotifierWatcher",
    default_service = "org.kde.StatusNotifierWatcher",
    default_path = "/StatusNotifierWatcher"
)]
trait StatusNotifierWatcherDbus {
    fn register_status_notifier_host(&self, service: &str) -> zbus::Result<()>;

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> zbus::Result<Vec<String>>;

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> zbus::Result<bool>;
}

/// State tracking whether the shell is registered with an active watcher.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WatcherRegistration {
    registered: bool,
    current_owner: Option<String>,
}

impl WatcherRegistration {
    /// Creates a new uninitialised registration state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            registered: false,
            current_owner: None,
        }
    }

    /// Whether the shell is currently registered with an active watcher.
    #[must_use]
    pub const fn is_registered(&self) -> bool {
        self.registered
    }

    /// The current watcher owner's unique bus name, if known.
    #[must_use]
    pub fn current_owner(&self) -> Option<&str> {
        self.current_owner.as_deref()
    }

    /// Marks the shell as registered with the given watcher owner.
    pub fn mark_registered(&mut self, owner: Option<String>) {
        self.registered = true;
        self.current_owner = owner;
    }

    /// Marks the shell as unregistered.
    pub fn mark_unregistered(&mut self) {
        self.registered = false;
        self.current_owner = None;
    }

    /// Handles a D-Bus `NameOwnerChanged` signal for `org.kde.StatusNotifierWatcher`.
    ///
    /// Returns `true` if registration should be performed.
    ///
    /// - If `new_owner` is empty, the watcher disappeared; state is reset to
    ///   unregistered and returns `false`.
    /// - If `new_owner` is identical to the current registered owner, returns
    ///   `false` (repeated signals do not trigger duplicate registrations).
    /// - If `new_owner` is a new non-empty owner, returns `true`. Registering
    ///   with a replacement watcher daemon is registering once per watcher
    ///   instance, not twice with the same watcher, because the new daemon has
    ///   fresh state.
    pub fn handle_name_owner_changed(&mut self, _old_owner: &str, new_owner: &str) -> bool {
        if new_owner.is_empty() {
            self.mark_unregistered();
            return false;
        }

        if self.registered && self.current_owner.as_deref() == Some(new_owner) {
            return false;
        }

        true
    }
}

/// Real zbus adapter implementing [`StatusNotifierWatcher`] over D-Bus.
#[derive(Debug)]
pub struct ZbusWatcher {
    conn: zbus::Connection,
    deadline: Duration,
    registration: WatcherRegistration,
    owner_change_stream: Option<zbus::fdo::NameOwnerChangedStream>,
}

impl ZbusWatcher {
    /// Creates a new [`ZbusWatcher`] using [`WATCHER_DEADLINE`].
    #[must_use]
    pub const fn new(conn: zbus::Connection) -> Self {
        Self {
            conn,
            deadline: WATCHER_DEADLINE,
            registration: WatcherRegistration::new(),
            owner_change_stream: None,
        }
    }

    /// Creates a new [`ZbusWatcher`] with an explicit deadline.
    #[must_use]
    pub const fn with_deadline(conn: zbus::Connection, deadline: Duration) -> Self {
        Self {
            conn,
            deadline,
            registration: WatcherRegistration::new(),
            owner_change_stream: None,
        }
    }

    /// The current registration state.
    #[must_use]
    pub const fn registration(&self) -> &WatcherRegistration {
        &self.registration
    }

    /// Mutable reference to registration state.
    pub fn registration_mut(&mut self) -> &mut WatcherRegistration {
        &mut self.registration
    }

    /// The connection used by this watcher adapter.
    #[must_use]
    pub const fn connection(&self) -> &zbus::Connection {
        &self.conn
    }

    /// Subscribes to `NameOwnerChanged` signals for `org.kde.StatusNotifierWatcher`.
    ///
    /// Stream must be created before the watcher appears to guarantee no signal is lost.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] if match rule registration fails.
    pub async fn subscribe_owner_changes_async(&mut self) -> Result<(), PlatformError> {
        let dbus = zbus::fdo::DBusProxy::new(&self.conn)
            .await
            .map_err(|e| map_dbus_err(Call::Subscribe, &e))?;
        let stream = dbus
            .receive_name_owner_changed_with_args(&[(0, WATCHER_SERVICE)])
            .await
            .map_err(|e| map_dbus_err(Call::Subscribe, &e))?;
        self.owner_change_stream = Some(stream);
        Ok(())
    }

    /// Synchronous variant of [`subscribe_owner_changes_async`](Self::subscribe_owner_changes_async).
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] if match rule registration fails.
    pub fn subscribe_owner_changes(&mut self) -> Result<(), PlatformError> {
        async_io::block_on(self.subscribe_owner_changes_async())
    }

    /// Waits for at most one `NameOwnerChanged` signal under the given timeout.
    ///
    /// Stream must be created before the watcher appears (e.g. by calling
    /// [`subscribe_owner_changes`](Self::subscribe_owner_changes)) so no signal is lost. If not already subscribed,
    /// this function will subscribe before waiting.
    ///
    /// Returns `Ok(Some(true))` if a signal was received and resulted in host registration,
    /// `Ok(Some(false))` if a signal was received but no registration was required
    /// (e.g. watcher disappeared or duplicate owner signal), and `Ok(None)` if the
    /// deadline elapsed with no signal.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] if subscription, D-Bus decoding, or registration fails.
    pub async fn next_owner_change_async(
        &mut self,
        wait: Duration,
    ) -> Result<Option<bool>, PlatformError> {
        if self.owner_change_stream.is_none() {
            self.subscribe_owner_changes_async().await?;
        }
        let Some(stream) = self.owner_change_stream.as_mut() else {
            return Ok(None);
        };

        if wait.is_zero() {
            return Ok(None);
        }

        let timer = async {
            async_io::Timer::after(wait).await;
            SignalEvent::Timeout
        };
        let receive = async {
            match stream.next().await {
                Some(s) => SignalEvent::Signal(s),
                None => SignalEvent::Closed,
            }
        };

        match futures_lite::future::or(receive, timer).await {
            SignalEvent::Timeout | SignalEvent::Closed => Ok(None),
            SignalEvent::Signal(signal) => {
                let args = signal
                    .args()
                    .map_err(|e| map_dbus_err(Call::Subscribe, &e))?;
                if args.name().as_str() != WATCHER_SERVICE {
                    return Ok(Some(false));
                }
                let old_owner = args.old_owner().as_ref().map_or("", |u| u.as_str());
                let new_owner = args.new_owner().as_ref().map_or("", |u| u.as_str());
                let registered = self
                    .handle_name_owner_changed_async(old_owner, new_owner)
                    .await?;
                Ok(Some(registered))
            }
        }
    }

    /// Synchronous variant of [`next_owner_change_async`](Self::next_owner_change_async).
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] on communication or registration failure.
    pub fn next_owner_change(&mut self, wait: Duration) -> Result<Option<bool>, PlatformError> {
        async_io::block_on(self.next_owner_change_async(wait))
    }

    /// Handles a `NameOwnerChanged` signal for `org.kde.StatusNotifierWatcher`.
    ///
    /// If the watcher appears and registration is required, attempts registration
    /// of [`HOST_SERVICE`].
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] if registration call fails.
    pub fn handle_name_owner_changed(
        &mut self,
        old_owner: &str,
        new_owner: &str,
    ) -> Result<bool, PlatformError> {
        async_io::block_on(self.handle_name_owner_changed_async(old_owner, new_owner))
    }

    /// Asynchronous variant of [`handle_name_owner_changed`](Self::handle_name_owner_changed).
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] if registration call fails.
    pub async fn handle_name_owner_changed_async(
        &mut self,
        old_owner: &str,
        new_owner: &str,
    ) -> Result<bool, PlatformError> {
        if self
            .registration
            .handle_name_owner_changed(old_owner, new_owner)
        {
            self.register_host_async(HOST_SERVICE).await?;
            self.registration
                .mark_registered(Some(new_owner.to_owned()));
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Asynchronous host registration.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] on deadline, refusal, or missing watcher.
    pub async fn register_host_async(&mut self, service: &str) -> Result<(), PlatformError> {
        self.acquire_host_name(service).await?;
        self.call_register_host(service).await?;
        self.check_host_registered().await?;
        self.update_registration().await;
        Ok(())
    }

    /// Asynchronous item query.
    ///
    /// # Errors
    ///
    /// Returns [`PlatformError`] on deadline, refusal, or missing watcher.
    pub async fn registered_items_async(&self) -> Result<Vec<String>, PlatformError> {
        let proxy = self.make_proxy().await?;
        let items = with_deadline(
            Call::RegisteredItems,
            self.deadline,
            proxy.registered_status_notifier_items(),
        )
        .await
        .map_err(|e| map_call_err(Call::RegisteredItems, e))?;
        Ok(items)
    }

    async fn acquire_host_name(&self, service: &str) -> Result<(), PlatformError> {
        let well_known =
            WellKnownName::try_from(service).map_err(|_| PlatformError::Refused("request_name"))?;
        let req = self.conn.request_name(well_known);
        with_deadline(Call::RequestName, self.deadline, req)
            .await
            .map_err(|e| map_call_err(Call::RequestName, e))?;
        Ok(())
    }

    async fn call_register_host(&self, service: &str) -> Result<(), PlatformError> {
        let proxy = self.make_proxy().await?;
        with_deadline(
            Call::RegisterHost,
            self.deadline,
            proxy.register_status_notifier_host(service),
        )
        .await
        .map_err(|e| map_call_err(Call::RegisterHost, e))?;
        Ok(())
    }

    async fn check_host_registered(&self) -> Result<(), PlatformError> {
        let proxy = self.make_proxy().await?;
        let registered = with_deadline(
            Call::IsHostRegistered,
            self.deadline,
            proxy.is_status_notifier_host_registered(),
        )
        .await
        .map_err(|e| map_call_err(Call::IsHostRegistered, e))?;

        if registered {
            Ok(())
        } else {
            Err(PlatformError::Refused("IsStatusNotifierHostRegistered"))
        }
    }

    async fn update_registration(&mut self) {
        let owner = query_watcher_owner(&self.conn).await;
        self.registration.mark_registered(owner);
    }

    async fn make_proxy(&self) -> Result<StatusNotifierWatcherDbusProxy<'_>, PlatformError> {
        StatusNotifierWatcherDbusProxy::builder(&self.conn)
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .map_err(|e| map_dbus_err(Call::RegisterHost, &e))
    }
}

impl StatusNotifierWatcher for ZbusWatcher {
    fn register_host(&mut self, service: &str) -> Result<(), PlatformError> {
        async_io::block_on(self.register_host_async(service))
    }

    fn registered_items(&self) -> Result<Vec<String>, PlatformError> {
        async_io::block_on(self.registered_items_async())
    }
}

async fn query_watcher_owner(conn: &zbus::Connection) -> Option<String> {
    let dbus = zbus::fdo::DBusProxy::new(conn).await.ok()?;
    let name = WellKnownName::from_static_str(WATCHER_SERVICE).ok()?;
    dbus.get_name_owner(name.into())
        .await
        .ok()
        .map(|o| o.to_string())
}

async fn with_deadline<T>(
    call: Call,
    d: Duration,
    fut: impl std::future::Future<Output = zbus::Result<T>>,
) -> Result<T, WatcherCallError> {
    if d.is_zero() {
        return Err(WatcherCallError::Deadline(call));
    }
    let timer = async {
        async_io::Timer::after(d).await;
        Err(WatcherCallError::Deadline(call))
    };
    let work = async { fut.await.map_err(WatcherCallError::Bus) };
    futures_lite::future::or(work, timer).await
}

fn map_call_err(call: Call, err: WatcherCallError) -> PlatformError {
    match err {
        WatcherCallError::Deadline(c) => PlatformError::Unavailable(c.deadline_text()),
        WatcherCallError::Bus(e) => map_dbus_err(call, &e),
    }
}

fn map_dbus_err(call: Call, err: &zbus::Error) -> PlatformError {
    if is_absent_watcher(err) {
        PlatformError::Unavailable(WATCHER_SERVICE)
    } else {
        PlatformError::Refused(call.name())
    }
}

fn is_absent_watcher(err: &zbus::Error) -> bool {
    match err {
        zbus::Error::MethodError(name, _, _) => {
            let s = name.as_str();
            s == "org.freedesktop.DBus.Error.ServiceUnknown"
                || s == "org.freedesktop.DBus.Error.NameHasNoOwner"
        }
        zbus::Error::FDO(fdo_err) => matches!(
            **fdo_err,
            zbus::fdo::Error::ServiceUnknown(_) | zbus::fdo::Error::NameHasNoOwner(_)
        ),
        _ => false,
    }
}
