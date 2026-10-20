// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `StatusNotifierWatcher` integration tests on a private D-Bus daemon (M29 E29-3).
//!
//! Spawns a dedicated private `dbus-daemon --session --print-address --nofork`,
//! serves a mock watcher implementing `org.kde.StatusNotifierWatcher`, and exercises
//! positive registration, negative absent-watcher refusal, and boundary late-appearance
//! deduplication.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use accesskit::TreeUpdate;
use aegis_forum_shell::canvas::seed;
use aegis_forum_shell::platform::{
    AccessibilityBus, CompositorLink, HOST_SERVICE, MountError, MountReport, Platform,
    PlatformError, PortalSettings, StatusNotifierWatcher, mount,
};
use aegis_forum_shell::state::ShellState;
use aegis_forum_shell_gpui::watcher::{
    WATCHER_DEADLINE, WATCHER_INTERFACE, WATCHER_PATH, WATCHER_SERVICE, WatcherRegistration,
    ZbusWatcher,
};

struct PrivateBus {
    child: std::process::Child,
    address: String,
}

impl PrivateBus {
    #[expect(
        clippy::print_stdout,
        reason = "HISS-21 / gate contract requires explicit printed SKIP when dbus-daemon absent"
    )]
    fn new() -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let mut child = match std::process::Command::new("dbus-daemon")
            .args(["--session", "--print-address", "--nofork"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                println!("SKIP: dbus-daemon absent");
                return Ok(None);
            }
            Err(e) => return Err(e.into()),
        };

        let stdout = child.stdout.take().ok_or("missing stdout")?;
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = std::thread::spawn(move || {
            use std::io::BufRead;
            let mut reader = std::io::BufReader::new(stdout);
            let mut line = String::new();
            let res = reader.read_line(&mut line).map(|_| line);
            let _ = tx.send(res);
        });

        let line = match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(line)) => line.trim().to_owned(),
            Ok(Err(e)) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.into());
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("dbus-daemon startup timed out".into());
            }
        };

        if line.is_empty() {
            let _ = child.kill();
            let _ = child.wait();
            return Err("dbus-daemon returned empty address".into());
        }

        Ok(Some(Self {
            child,
            address: line,
        }))
    }

    fn address(&self) -> &str {
        &self.address
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Clone, Default)]
struct TestWatcherState {
    hosts: Arc<Mutex<Vec<String>>>,
    items: Arc<Mutex<Vec<String>>>,
}

impl TestWatcherState {
    fn add_item(&self, item: String) -> Result<(), Box<dyn std::error::Error>> {
        let mut items = self
            .items
            .lock()
            .map_err(|_| "items mutex poisoned".to_owned())?;
        items.push(item);
        Ok(())
    }

    fn hosts_list(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let hosts = self
            .hosts
            .lock()
            .map_err(|_| "hosts mutex poisoned".to_owned())?;
        Ok(hosts.clone())
    }
}

struct TestWatcherService {
    state: TestWatcherState,
}

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl TestWatcherService {
    fn register_status_notifier_host(&self, service: String) -> zbus::fdo::Result<()> {
        let mut hosts = self
            .state
            .hosts
            .lock()
            .map_err(|_| zbus::fdo::Error::Failed("poison".to_owned()))?;
        hosts.push(service);
        Ok(())
    }

    fn register_status_notifier_item(&self, service: String) -> zbus::fdo::Result<()> {
        let mut items = self
            .state
            .items
            .lock()
            .map_err(|_| zbus::fdo::Error::Failed("poison".to_owned()))?;
        items.push(service);
        Ok(())
    }

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.state
            .items
            .lock()
            .map_or_else(|_| Vec::new(), |i| i.clone())
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        self.state.hosts.lock().is_ok_and(|h| !h.is_empty())
    }

    #[expect(clippy::unused_self, reason = "zbus property method requires &self")]
    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
}

struct MockCompositor;
impl CompositorLink for MockCompositor {
    fn sync_desktop_shell(&mut self) -> Result<(), PlatformError> {
        Ok(())
    }
}

struct MockPortal;
impl PortalSettings for MockPortal {
    fn read_u32(&self, _namespace: &str, _key: &str) -> Result<Option<u32>, PlatformError> {
        Ok(None)
    }
}

struct MockBus;
impl AccessibilityBus for MockBus {
    fn publish(&mut self, _update: &TreeUpdate) -> Result<(), PlatformError> {
        Ok(())
    }
}

async fn client(bus: &PrivateBus) -> Result<zbus::Connection, Box<dyn std::error::Error>> {
    let conn = zbus::connection::Builder::address(bus.address())?
        .build()
        .await?;
    Ok(conn)
}

async fn watcher_on(
    bus: &PrivateBus,
    deadline: Duration,
) -> Result<ZbusWatcher, Box<dyn std::error::Error>> {
    let conn = client(bus).await?;
    Ok(ZbusWatcher::with_deadline(conn, deadline))
}

fn watcher_sync(
    bus: &PrivateBus,
    deadline: Duration,
) -> Result<ZbusWatcher, Box<dyn std::error::Error>> {
    async_io::block_on(watcher_on(bus, deadline))
}

async fn serve_watcher(
    bus: &PrivateBus,
    state: TestWatcherState,
) -> Result<zbus::Connection, Box<dyn std::error::Error>> {
    let conn = zbus::connection::Builder::address(bus.address())?
        .name(WATCHER_SERVICE)?
        .serve_at(WATCHER_PATH, TestWatcherService { state })?
        .build()
        .await?;
    Ok(conn)
}

fn mount_with(watcher: &mut ZbusWatcher) -> Result<MountReport, MountError> {
    let canvas =
        seed::grid(10, 2).map_err(|_e| MountError::Watcher(PlatformError::Unavailable("grid")))?;
    let mut shell_state = ShellState::new(canvas);
    let mut compositor = MockCompositor;
    let portal = MockPortal;
    let mut bus_mock = MockBus;

    let mut platform = Platform {
        compositor: &mut compositor,
        watcher,
        portal: &portal,
        bus: &mut bus_mock,
    };

    mount(&mut shell_state, &mut platform)
}

/// Positive: on mount the shell registers with `StatusNotifierWatcher`, and the watcher lists it.
#[test]
fn positive_watcher_registration_and_listing() -> Result<(), Box<dyn std::error::Error>> {
    let Some(bus) = PrivateBus::new()? else {
        return Ok(());
    };

    assert_eq!(WATCHER_SERVICE, "org.kde.StatusNotifierWatcher");
    assert_eq!(WATCHER_PATH, "/StatusNotifierWatcher");
    assert_eq!(WATCHER_INTERFACE, "org.kde.StatusNotifierWatcher");
    assert_eq!(WATCHER_DEADLINE, Duration::from_secs(2));

    async_io::block_on(async {
        let state = TestWatcherState::default();
        state.add_item("org.kde.StatusNotifierItem-1/StatusNotifierItem".to_owned())?;

        let server_conn = serve_watcher(&bus, state.clone()).await?;
        let mut watcher = watcher_on(&bus, WATCHER_DEADLINE).await?;
        assert!(!watcher.connection().is_closed());

        let report = mount_with(&mut watcher).map_err(|e| format!("mount failed: {e}"))?;

        assert_eq!(report.tray_items, 1);
        assert!(watcher.registration().is_registered());
        let recorded = state.hosts_list()?;
        assert_eq!(recorded, vec![HOST_SERVICE]);

        let items = watcher.registered_items()?;
        assert_eq!(
            items,
            vec!["org.kde.StatusNotifierItem-1/StatusNotifierItem"]
        );

        drop(server_conn);
        Ok(())
    })
}

/// Negative: with no watcher on the bus the registration fails with a typed error.
#[test]
fn negative_absent_watcher_fails_with_typed_error() -> Result<(), Box<dyn std::error::Error>> {
    let Some(bus) = PrivateBus::new()? else {
        return Ok(());
    };

    let mut watcher = watcher_sync(&bus, WATCHER_DEADLINE)?;

    let reg_res = watcher.register_host(HOST_SERVICE);
    assert_eq!(
        reg_res,
        Err(PlatformError::Unavailable("org.kde.StatusNotifierWatcher"))
    );
    assert!(!watcher.registration().is_registered());

    let items_res = watcher.registered_items();
    assert_eq!(
        items_res,
        Err(PlatformError::Unavailable("org.kde.StatusNotifierWatcher"))
    );

    let mount_res = mount_with(&mut watcher);
    assert_eq!(
        mount_res,
        Err(MountError::Watcher(PlatformError::Unavailable(
            "org.kde.StatusNotifierWatcher"
        )))
    );

    Ok(())
}

fn private_bus_client() -> Result<Option<(PrivateBus, zbus::Connection)>, Box<dyn std::error::Error>>
{
    let Some(bus) = PrivateBus::new()? else {
        return Ok(None);
    };
    let conn = async_io::block_on(client(&bus))?;
    Ok(Some((bus, conn)))
}

/// Negative & boundary: a zero deadline fails immediately naming the call.
#[test]
fn negative_zero_deadline_fails_immediately() -> Result<(), Box<dyn std::error::Error>> {
    let Some((_bus, conn)) = private_bus_client()? else {
        return Ok(());
    };

    let mut watcher = ZbusWatcher::with_deadline(conn, Duration::ZERO);

    let Err(err) = watcher.register_host(HOST_SERVICE) else {
        return Err("expected deadline error, got Ok".into());
    };
    match err {
        PlatformError::Unavailable(text) => {
            assert!(
                text.contains("deadline"),
                "expected deadline in error text, got {text}"
            );
        }
        PlatformError::Refused(text) => {
            return Err(format!("expected Unavailable, got Refused({text})").into());
        }
    }

    let Err(items_err) = watcher.registered_items() else {
        return Err("expected deadline error, got Ok".into());
    };
    match items_err {
        PlatformError::Unavailable(text) => {
            assert_eq!(text, "RegisteredStatusNotifierItems deadline");
        }
        PlatformError::Refused(text) => {
            return Err(format!("expected Unavailable, got Refused({text})").into());
        }
    }

    Ok(())
}

/// Serves a fresh test watcher and checks the shell registers with it from the owner signal.
async fn appear_and_register(
    watcher: &mut ZbusWatcher,
    bus: &PrivateBus,
    state: &TestWatcherState,
) -> Result<zbus::Connection, Box<dyn std::error::Error>> {
    let server_conn = serve_watcher(bus, state.clone()).await?;
    let owner = server_conn
        .unique_name()
        .ok_or("missing unique name")?
        .as_str()
        .to_owned();
    let change = watcher.next_owner_change(Duration::from_secs(2))?;
    assert_eq!(change, Some(true));
    assert!(watcher.registration().is_registered());
    assert_eq!(watcher.registration().current_owner(), Some(owner.as_str()));
    Ok(server_conn)
}

/// Boundary: a watcher that appears after the shell mounts is registered with once, not twice.
#[test]
fn boundary_late_watcher_registers_once_not_twice() -> Result<(), Box<dyn std::error::Error>> {
    let Some(bus) = PrivateBus::new()? else {
        return Ok(());
    };

    async_io::block_on(async {
        let mut watcher = watcher_on(&bus, WATCHER_DEADLINE).await?;
        assert!(!watcher.registration().is_registered());

        watcher.subscribe_owner_changes()?;

        let state = TestWatcherState::default();
        let server_conn1 = appear_and_register(&mut watcher, &bus, &state).await?;
        assert_eq!(state.hosts_list()?.as_slice(), [HOST_SERVICE]);

        let second_change = watcher.next_owner_change(Duration::from_millis(50))?;
        assert_eq!(second_change, None);
        assert_eq!(state.hosts_list()?.as_slice(), [HOST_SERVICE]);

        drop(server_conn1);
        let drop_change = watcher.next_owner_change(Duration::from_secs(2))?;
        assert_eq!(drop_change, Some(false));
        assert!(!watcher.registration().is_registered());
        assert_eq!(watcher.registration().current_owner(), None);

        let server_conn2 = appear_and_register(&mut watcher, &bus, &state).await?;
        assert_eq!(state.hosts_list()?.len(), 2);
        let owner2 = server_conn2
            .unique_name()
            .ok_or("missing unique name")?
            .as_str()
            .to_owned();

        let did_handle_duplicate = watcher.handle_name_owner_changed(&owner2, &owner2)?;
        assert!(!did_handle_duplicate);

        watcher.registration_mut().mark_unregistered();
        assert!(!watcher.registration().is_registered());

        drop(server_conn2);
        Ok(())
    })
}

fn check_registration_initial(reg: &mut WatcherRegistration) {
    assert!(!reg.is_registered());
    assert_eq!(reg.current_owner(), None);
    assert!(!reg.handle_name_owner_changed("", ""));
    assert!(!reg.is_registered());
}

fn check_registration_appear(reg: &mut WatcherRegistration) {
    assert!(reg.handle_name_owner_changed("", ":1.10"));
    assert!(!reg.is_registered());

    reg.mark_registered(Some(":1.10".to_owned()));
    assert!(reg.is_registered());
    assert_eq!(reg.current_owner(), Some(":1.10"));

    assert!(!reg.handle_name_owner_changed("", ":1.10"));
    assert!(!reg.handle_name_owner_changed(":1.10", ":1.10"));
    assert!(reg.is_registered());
}

fn check_registration_owner_change_and_disappear(reg: &mut WatcherRegistration) {
    assert!(reg.handle_name_owner_changed(":1.10", ":1.20"));
    reg.mark_registered(Some(":1.20".to_owned()));
    assert_eq!(reg.current_owner(), Some(":1.20"));

    assert!(!reg.handle_name_owner_changed(":1.20", ""));
    assert!(!reg.is_registered());
    assert_eq!(reg.current_owner(), None);

    assert!(reg.handle_name_owner_changed("", ":1.30"));
}

/// Boundary: pure `WatcherRegistration` state transition unit checks.
#[test]
fn boundary_watcher_registration_unit_lifecycle() {
    let mut reg = WatcherRegistration::new();
    check_registration_initial(&mut reg);
    check_registration_appear(&mut reg);
    check_registration_owner_change_and_disappear(&mut reg);
}
