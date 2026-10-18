// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Readback deadline unit tests (M28 E28-3): `with_deadline` behavior on silent peer.

use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use aegis_forum_shell_gpui::readback::{ReadError, with_deadline};

struct Silent;

#[zbus::interface(name = "org.a11y.atspi.Accessible")]
impl Silent {
    async fn get_role(&self) -> atspi::Role {
        std::future::pending::<()>().await;
        atspi::Role::Invalid
    }

    #[expect(clippy::unused_self, reason = "zbus property method requires &self")]
    #[zbus(property)]
    fn name(&self) -> String {
        "x".to_owned()
    }
}

async fn check_answering_call(
    proxy: &atspi::proxy::accessible::AccessibleProxy<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let name_res = with_deadline("Name", Duration::from_millis(1000), proxy.name()).await;
    match name_res {
        Ok(name) => assert_eq!(name, "x"),
        Err(err) => return Err(format!("expected method to answer, got {err}").into()),
    }
    Ok(())
}

async fn check_hanging_call(
    proxy: &atspi::proxy::accessible::AccessibleProxy<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let hang_res = with_deadline("GetRole", Duration::from_millis(300), proxy.get_role()).await;
    let elapsed = start.elapsed();
    match hang_res {
        Err(ReadError::Deadline { call, ms }) => {
            assert_eq!(call, "GetRole");
            assert_eq!(ms, 300);
        }
        Ok(_) => return Err("expected deadline timeout, but method succeeded".into()),
        Err(err) => return Err(format!("expected ReadError::Deadline, got {err}").into()),
    }
    assert!(
        elapsed >= Duration::from_millis(250) && elapsed < Duration::from_millis(2000),
        "elapsed time {elapsed:?} outside 250..2000 ms range"
    );
    Ok(())
}

async fn check_zero_deadline(
    proxy: &atspi::proxy::accessible::AccessibleProxy<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let zero_res = with_deadline("NameZero", Duration::ZERO, proxy.name()).await;
    assert!(
        matches!(zero_res, Err(ReadError::Deadline { ref call, .. }) if call == "NameZero"),
        "expected 0 ms deadline to fail immediately, got {zero_res:?}"
    );
    Ok(())
}

type Proxy = atspi::proxy::accessible::AccessibleProxy<'static>;

async fn connect_silent_peer() -> Result<(zbus::Connection, Proxy), Box<dyn std::error::Error>> {
    let (a, b) = UnixStream::pair()?;
    let guid = zbus::Guid::generate();

    let server_builder = zbus::connection::Builder::async_io_unix_stream(a)
        .server(guid)?
        .p2p()
        .serve_at("/org/a11y/atspi/accessible/root", Silent)?;

    let client_builder = zbus::connection::Builder::async_io_unix_stream(b).p2p();

    let (server_conn_res, client_conn_res) =
        futures_lite::future::zip(server_builder.build(), client_builder.build()).await;
    let server_conn = server_conn_res?;
    let client_conn = client_conn_res?;

    let proxy = atspi::proxy::accessible::AccessibleProxy::builder(&client_conn)
        .destination("org.a11y.atspi.Accessible")?
        .path("/org/a11y/atspi/accessible/root")?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await?;

    Ok((server_conn, proxy))
}

type Check = for<'a> fn(
    &'a Proxy,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<(), Box<dyn std::error::Error>>> + 'a>,
>;

fn run_against_silent_peer(check: Check) -> Result<(), Box<dyn std::error::Error>> {
    async_io::block_on(async {
        let battery = async {
            let (_server, proxy) = connect_silent_peer().await?;
            check(&proxy).await
        };
        with_deadline("outer_test_deadline", Duration::from_secs(10), battery).await
    })?;
    Ok(())
}

/// Positive: a call the peer answers passes through `with_deadline` with its value.
#[test]
fn answering_call_passes_through() -> Result<(), Box<dyn std::error::Error>> {
    run_against_silent_peer(|p| Box::pin(check_answering_call(p)))
}

/// Negative: a call the peer never answers fails at its deadline, naming the call.
#[test]
fn silent_call_fails_at_deadline() -> Result<(), Box<dyn std::error::Error>> {
    run_against_silent_peer(|p| Box::pin(check_hanging_call(p)))
}

/// Boundary: a zero deadline fails at once, before the call is sent.
#[test]
fn zero_deadline_fails_immediately() -> Result<(), Box<dyn std::error::Error>> {
    run_against_silent_peer(|p| Box::pin(check_zero_deadline(p)))
}
