// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! AT-SPI tree readback client over D-Bus (M28 E28-3).
//!
//! Connects to the private accessibility D-Bus, discovers the fixture application,
//! traverses the accessible tree using a bounded breadth-first walk, and performs
//! interactive action triggering with deadline enforcement on every remote call.

use std::collections::VecDeque;
use std::time::Duration;

use atspi::object_ref::ObjectRefOwned;
use atspi::proxy::accessible::AccessibleProxy;
use atspi::proxy::action::ActionProxy;
use atspi::proxy::bus::BusProxy;
use atspi::proxy::component::ComponentProxy;
use zbus::names::BusName;

use crate::fixture::{BUTTON_LABEL_AFTER, FIXTURE_TITLE};
use crate::judge::{CaseResult, ReadNode};

/// Maximum duration permitted for an individual D-Bus roundtrip.
pub const CALL_DEADLINE: Duration = Duration::from_secs(2);

/// Maximum duration permitted to locate the fixture on the registry bus.
pub const APP_DEADLINE: Duration = Duration::from_secs(15);

/// Maximum duration permitted to observe a post-click incremental update.
pub const UPDATE_DEADLINE: Duration = Duration::from_secs(5);

/// Hard upper bound on the number of nodes visited during tree traversal.
pub const MAX_NODES: usize = 5000;

/// Errors arising during AT-SPI readback operations.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    /// A D-Bus method call or property read exceeded its allotted deadline.
    #[error("D-Bus call '{call}' timed out after {ms} ms")]
    Deadline {
        /// Name of the operation that timed out.
        call: String,
        /// Configured deadline duration in milliseconds.
        ms: u128,
    },
    /// An underlying D-Bus protocol or transport failure occurred.
    #[error("D-Bus error: {0}")]
    Bus(String),
    /// An expected accessible object or application could not be located.
    #[error("AT-SPI target not found: {0}")]
    NotFound(String),
    /// Tree traversal visited more nodes than the safety bound.
    #[error("tree node bound exceeded: {0} nodes")]
    Bound(usize),
}

/// Executes a future under an explicit deadline, mapping timeouts to [`ReadError::Deadline`].
///
/// # Errors
///
/// Returns [`ReadError::Deadline`] if `d` elapses before `fut` resolves, or [`ReadError::Bus`]
/// if `fut` yields an error.
pub async fn with_deadline<T, E: std::fmt::Display>(
    what: &str,
    d: Duration,
    fut: impl std::future::Future<Output = Result<T, E>>,
) -> Result<T, ReadError> {
    if d.is_zero() {
        return Err(ReadError::Deadline {
            call: what.to_owned(),
            ms: 0,
        });
    }
    let call_name = what.to_owned();
    let ms = d.as_millis();
    let timer = async {
        async_io::Timer::after(d).await;
        Err(ReadError::Deadline {
            call: call_name,
            ms,
        })
    };
    let work = async { fut.await.map_err(|err| ReadError::Bus(err.to_string())) };
    futures_lite::future::or(work, timer).await
}

/// Connects to the accessibility message bus queried from the session bus.
///
/// # Errors
///
/// Returns [`ReadError`] if session bus connection, address lookup, or a11y connection fails.
pub async fn a11y_connection() -> Result<zbus::Connection, ReadError> {
    let session_conn =
        with_deadline("session_bus", CALL_DEADLINE, zbus::Connection::session()).await?;
    let bus_proxy =
        with_deadline("bus_proxy_new", CALL_DEADLINE, BusProxy::new(&session_conn)).await?;
    let a11y_addr = with_deadline(
        "bus_proxy_get_address",
        CALL_DEADLINE,
        bus_proxy.get_address(),
    )
    .await?;
    let a11y_builder = zbus::connection::Builder::address(a11y_addr.as_str())
        .map_err(|e| ReadError::Bus(e.to_string()))?;
    let a11y_conn = with_deadline("a11y_bus_build", CALL_DEADLINE, a11y_builder.build()).await?;
    Ok(a11y_conn)
}

async fn make_accessible_proxy<'a>(
    conn: &'a zbus::Connection,
    obj: &ObjectRefOwned,
) -> Result<AccessibleProxy<'a>, ReadError> {
    let name: BusName = obj
        .name()
        .ok_or_else(|| ReadError::Bus("missing bus name on ObjectRef".to_owned()))?
        .clone()
        .into();
    let path = obj.path();
    let builder = AccessibleProxy::builder(conn)
        .destination(name)
        .map_err(|e| ReadError::Bus(e.to_string()))?
        .path(path)
        .map_err(|e| ReadError::Bus(e.to_string()))?
        .cache_properties(zbus::proxy::CacheProperties::No);
    with_deadline("accessible_proxy_build", CALL_DEADLINE, builder.build()).await
}

async fn make_action_proxy<'a>(
    conn: &'a zbus::Connection,
    obj: &ObjectRefOwned,
) -> Result<ActionProxy<'a>, ReadError> {
    let name: BusName = obj
        .name()
        .ok_or_else(|| ReadError::Bus("missing bus name on ObjectRef".to_owned()))?
        .clone()
        .into();
    let path = obj.path();
    let builder = ActionProxy::builder(conn)
        .destination(name)
        .map_err(|e| ReadError::Bus(e.to_string()))?
        .path(path)
        .map_err(|e| ReadError::Bus(e.to_string()))?
        .cache_properties(zbus::proxy::CacheProperties::No);
    with_deadline("action_proxy_build", CALL_DEADLINE, builder.build()).await
}

async fn make_component_proxy<'a>(
    conn: &'a zbus::Connection,
    obj: &ObjectRefOwned,
) -> Result<ComponentProxy<'a>, ReadError> {
    let name: BusName = obj
        .name()
        .ok_or_else(|| ReadError::Bus("missing bus name on ObjectRef".to_owned()))?
        .clone()
        .into();
    let path = obj.path();
    let builder = ComponentProxy::builder(conn)
        .destination(name)
        .map_err(|e| ReadError::Bus(e.to_string()))?
        .path(path)
        .map_err(|e| ReadError::Bus(e.to_string()))?
        .cache_properties(zbus::proxy::CacheProperties::No);
    with_deadline("component_proxy_build", CALL_DEADLINE, builder.build()).await
}

async fn check_win_for_fixture(conn: &zbus::Connection, win_ref: &ObjectRefOwned) -> bool {
    let Ok(win_proxy) = make_accessible_proxy(conn, win_ref).await else {
        return false;
    };
    let Ok(title) = with_deadline("win_name", CALL_DEADLINE, win_proxy.name()).await else {
        return false;
    };
    title == FIXTURE_TITLE
}

async fn check_app_for_fixture(
    conn: &zbus::Connection,
    app_ref: &ObjectRefOwned,
) -> Result<bool, ReadError> {
    let app_proxy = make_accessible_proxy(conn, app_ref).await?;
    let win_refs =
        with_deadline("app_get_children", CALL_DEADLINE, app_proxy.get_children()).await?;
    for win_ref in &win_refs {
        if check_win_for_fixture(conn, win_ref).await {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn scan_registry_for_fixture(
    conn: &zbus::Connection,
    reg_proxy: &AccessibleProxy<'_>,
) -> Result<Option<ObjectRefOwned>, ReadError> {
    let Ok(app_refs) = with_deadline(
        "registry_get_children",
        CALL_DEADLINE,
        reg_proxy.get_children(),
    )
    .await
    else {
        return Ok(None);
    };
    for app_ref in app_refs {
        if check_app_for_fixture(conn, &app_ref).await.unwrap_or(false) {
            return Ok(Some(app_ref));
        }
    }
    Ok(None)
}

/// Polls the accessibility registry until the fixture application is registered.
///
/// # Errors
///
/// Returns [`ReadError::NotFound`] if the fixture is not found before `deadline` expires.
pub async fn find_fixture(
    conn: &zbus::Connection,
    deadline: Duration,
) -> Result<ObjectRefOwned, ReadError> {
    let reg_builder = AccessibleProxy::builder(conn)
        .destination("org.a11y.atspi.Registry")
        .map_err(|e| ReadError::Bus(e.to_string()))?
        .path("/org/a11y/atspi/accessible/root")
        .map_err(|e| ReadError::Bus(e.to_string()))?
        .cache_properties(zbus::proxy::CacheProperties::No);
    let reg_proxy = with_deadline("registry_proxy", CALL_DEADLINE, reg_builder.build()).await?;

    let max_iterations = (deadline.as_millis() / 200).max(1);
    for _ in 0..max_iterations {
        if let Some(app_ref) = scan_registry_for_fixture(conn, &reg_proxy).await? {
            return Ok(app_ref);
        }
        async_io::Timer::after(Duration::from_millis(200)).await;
    }
    Err(ReadError::NotFound(format!(
        "fixture app with window '{FIXTURE_TITLE}' not found within {deadline:?}"
    )))
}

async fn query_actions(
    conn: &zbus::Connection,
    obj_ref: &ObjectRefOwned,
    ifaces: atspi::InterfaceSet,
) -> Vec<String> {
    if !ifaces.contains(atspi::Interface::Action) {
        return Vec::new();
    }
    let Ok(act_proxy) = make_action_proxy(conn, obj_ref).await else {
        return Vec::new();
    };
    let Ok(acts) = with_deadline("get_actions", CALL_DEADLINE, act_proxy.get_actions()).await
    else {
        return Vec::new();
    };
    acts.into_iter().map(|a| a.name).collect()
}

async fn query_extents(
    conn: &zbus::Connection,
    obj_ref: &ObjectRefOwned,
    ifaces: atspi::InterfaceSet,
) -> Option<(i32, i32, i32, i32)> {
    if !ifaces.contains(atspi::Interface::Component) {
        return None;
    }
    let Ok(comp_proxy) = make_component_proxy(conn, obj_ref).await else {
        return None;
    };
    let ext_res = with_deadline(
        "get_extents",
        CALL_DEADLINE,
        comp_proxy.get_extents(atspi::CoordType::Window),
    )
    .await;
    ext_res.ok()
}

async fn extract_node_data(
    conn: &zbus::Connection,
    obj_ref: &ObjectRefOwned,
    path: Vec<usize>,
) -> Result<(ReadNode, Vec<ObjectRefOwned>), ReadError> {
    let proxy = make_accessible_proxy(conn, obj_ref).await?;
    let role = with_deadline("get_role", CALL_DEADLINE, proxy.get_role()).await?;
    let name = with_deadline("name", CALL_DEADLINE, proxy.name()).await?;
    let state_set = with_deadline("get_state", CALL_DEADLINE, proxy.get_state()).await?;
    let states = state_set
        .into_iter()
        .map(|s| s.to_static_str().to_owned())
        .collect();
    let ifaces = with_deadline("get_interfaces", CALL_DEADLINE, proxy.get_interfaces()).await?;

    let actions = query_actions(conn, obj_ref, ifaces).await;
    let extents = query_extents(conn, obj_ref, ifaces).await;

    let children = with_deadline("get_children", CALL_DEADLINE, proxy.get_children()).await?;
    let node = ReadNode {
        path,
        role: role.name().to_owned(),
        name,
        states,
        actions,
        extents,
    };
    Ok((node, children))
}

/// Reads the entire accessible hierarchy starting from `app` using bounded BFS.
///
/// # Errors
///
/// Returns [`ReadError::Bound`] if visited nodes exceed [`MAX_NODES`], or [`ReadError`]
/// if D-Bus reads fail.
pub async fn read_tree(
    conn: &zbus::Connection,
    app: &ObjectRefOwned,
) -> Result<Vec<ReadNode>, ReadError> {
    let mut queue = VecDeque::new();
    queue.push_back((app.clone(), Vec::new()));
    let mut nodes = Vec::new();

    while let Some((obj_ref, path)) = queue.pop_front() {
        if nodes.len() >= MAX_NODES {
            return Err(ReadError::Bound(MAX_NODES));
        }
        let (node, children) = extract_node_data(conn, &obj_ref, path.clone()).await?;
        nodes.push(node);

        for (child_idx, child_ref) in children.into_iter().enumerate() {
            let mut child_path = path.clone();
            child_path.push(child_idx);
            queue.push_back((child_ref, child_path));
        }
    }
    Ok(nodes)
}

/// Searches the accessible hierarchy starting from `app` for an object matching `target_name`.
///
/// # Errors
///
/// Returns [`ReadError::NotFound`] if no node carries `target_name`.
pub async fn find_node_by_name(
    conn: &zbus::Connection,
    app: &ObjectRefOwned,
    target_name: &str,
) -> Result<ObjectRefOwned, ReadError> {
    let mut queue = VecDeque::new();
    queue.push_back(app.clone());
    let mut count = 0usize;

    while let Some(obj_ref) = queue.pop_front() {
        count = count.saturating_add(1);
        if count > MAX_NODES {
            return Err(ReadError::Bound(MAX_NODES));
        }
        let proxy = make_accessible_proxy(conn, &obj_ref).await?;
        let name = with_deadline("name", CALL_DEADLINE, proxy.name()).await?;
        if name == target_name {
            return Ok(obj_ref);
        }
        let children = with_deadline("get_children", CALL_DEADLINE, proxy.get_children()).await?;
        for child in children {
            queue.push_back(child);
        }
    }
    Err(ReadError::NotFound(format!(
        "node with name '{target_name}' not found"
    )))
}

/// Dispatches the "click" action on `node` and polls for the expected label change.
///
/// # Errors
///
/// Returns [`ReadError`] if action execution or D-Bus property reads fail.
pub async fn click_and_await_update(
    conn: &zbus::Connection,
    node: &ObjectRefOwned,
    deadline: Duration,
) -> Result<CaseResult, ReadError> {
    let act_proxy = make_action_proxy(conn, node).await?;
    let acts = with_deadline("get_actions", CALL_DEADLINE, act_proxy.get_actions()).await?;

    let click_idx = acts.iter().position(|a| a.name == "click");
    let Some(idx) = click_idx else {
        return Ok(CaseResult {
            name: "atspi/incremental-update",
            passed: false,
            why: "action 'click' not found on node".to_owned(),
        });
    };

    let act_idx = i32::try_from(idx).map_err(|e| ReadError::Bus(e.to_string()))?;
    let action_res =
        with_deadline("do_action", CALL_DEADLINE, act_proxy.do_action(act_idx)).await?;
    if !action_res {
        return Ok(CaseResult {
            name: "atspi/incremental-update",
            passed: false,
            why: "do_action returned false".to_owned(),
        });
    }

    let button_proxy = make_accessible_proxy(conn, node).await?;
    let max_polls = (deadline.as_millis() / 100).max(1);
    let mut last_name = String::new();
    for _ in 0..max_polls {
        async_io::Timer::after(Duration::from_millis(100)).await;
        let current = with_deadline("name", CALL_DEADLINE, button_proxy.name()).await?;
        if current == BUTTON_LABEL_AFTER {
            return Ok(CaseResult {
                name: "atspi/incremental-update",
                passed: true,
                why: String::new(),
            });
        }
        last_name = current;
    }

    Ok(CaseResult {
        name: "atspi/incremental-update",
        passed: false,
        why: format!("name was still '{last_name}', want '{BUTTON_LABEL_AFTER}'"),
    })
}
