// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Surface unit tests (M28 E28-2): window options mapping, error classification,
//! line formatting, exit codes, canvas decoding, and viewport bounds.

use aegis_forum_shell_gpui::surface::{
    FIRST_CONFIGURE_DEADLINE, Outcome, SHELL_HEIGHT, classify_open_error, decode_canvas, exit_code,
    outcome_line, viewport, window_options,
};
use aegis_forum_shell_gpui::{LayerMapError, layer_shell_options};
use aegis_scaena::mount::{Edge, Margin, MountCanvas, MountRequest, encode_request};
use aegis_scaena::{LineBuffer, MountError};
use gpui::layer_shell::LayerShellNotSupportedError;
use gpui::{Bounds, Size, WindowBounds, WindowKind, point, px};

mod common;

use common::sample_mount_canvas;

fn sample_canvas() -> Result<MountCanvas, MountError> {
    sample_mount_canvas(
        Some(48),
        Some(Margin {
            top: 0,
            right: 0,
            bottom: 0,
            left: 0,
        }),
    )
}

// --- Positive -------------------------------------------------------------

/// Positive: [`window_options`] configures layer shell window bounds, titlebar, and kind.
#[test]
fn window_options_configures_layer_shell() -> Result<(), Box<dyn std::error::Error>> {
    let canvas = sample_canvas()?;
    let opts = window_options(&canvas)?;

    assert!(opts.titlebar.is_none());
    let expected_bounds = WindowBounds::Windowed(Bounds {
        origin: point(px(0.0), px(0.0)),
        size: Size::new(px(0.0), px(SHELL_HEIGHT)),
    });
    assert_eq!(opts.window_bounds, Some(expected_bounds));

    let expected_layer_opts = layer_shell_options(&canvas)?;
    assert_eq!(opts.kind, WindowKind::LayerShell(expected_layer_opts));
    match &opts.kind {
        WindowKind::LayerShell(ls) => assert_eq!(ls.namespace, canvas.namespace.as_str()),
        _ => return Err("expected WindowKind::LayerShell".into()),
    }
    Ok(())
}

/// Positive: [`outcome_line`] produces exact strings for all five variants.
#[test]
fn outcome_line_exact_strings() {
    let configured = Outcome::Configured {
        width: 1280,
        height: 48,
    };
    assert_eq!(outcome_line(&configured), "OUTCOME configured 1280x48");

    let missing = Outcome::LayerShellMissing("not supported".to_owned());
    assert_eq!(
        outcome_line(&missing),
        "OUTCOME layer-shell-missing: not supported"
    );

    let ms = FIRST_CONFIGURE_DEADLINE.as_millis();
    let expected_deadline =
        format!("OUTCOME deadline: zwlr_layer_surface_v1.configure did not arrive within {ms} ms");
    assert_eq!(outcome_line(&Outcome::Deadline), expected_deadline);

    let open_failed = Outcome::OpenFailed("connection reset".to_owned());
    assert_eq!(
        outcome_line(&open_failed),
        "OUTCOME open-failed: connection reset"
    );

    let refused = Outcome::Refused("invalid schema".to_owned());
    assert_eq!(outcome_line(&refused), "OUTCOME refused: invalid schema");
}

/// Positive: [`exit_code`] returns the exact contract status codes.
#[test]
fn exit_codes_contract() {
    assert_eq!(
        exit_code(&Outcome::Configured {
            width: 1280,
            height: 48
        }),
        0
    );
    assert_eq!(
        exit_code(&Outcome::LayerShellMissing("missing".to_owned())),
        4
    );
    assert_eq!(exit_code(&Outcome::Deadline), 5);
    assert_eq!(exit_code(&Outcome::OpenFailed("failed".to_owned())), 3);
    assert_eq!(exit_code(&Outcome::Refused("refused".to_owned())), 6);
}

/// Positive: [`viewport`] converts valid float sizes into Configured.
#[test]
fn viewport_positive() {
    let size = Size::new(px(1280.0), px(48.0));
    assert_eq!(
        viewport(size),
        Outcome::Configured {
            width: 1280,
            height: 48,
        }
    );
}

/// Positive: [`decode_canvas`] roundtrips with [`encode_request`].
#[test]
fn decode_canvas_roundtrips() -> Result<(), Box<dyn std::error::Error>> {
    let canvas = sample_canvas()?;
    let request = MountRequest { id: 1, canvas };
    let mut buffer = LineBuffer::new();
    let encoded = encode_request(&request, &mut buffer)?;
    let decoded = decode_canvas(encoded).map_err(|outcome| format!("{outcome:?}"))?;
    assert_eq!(decoded, canvas);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: [`classify_open_error`] maps [`LayerShellNotSupportedError`] to `LayerShellMissing`.
#[test]
fn classify_open_error_layer_shell_missing() {
    assert_eq!(
        classify_open_error(&LayerShellNotSupportedError),
        Outcome::LayerShellMissing(
            "LayerShellNotSupportedError: Compositor doesn't support zwlr_layer_shell_v1"
                .to_owned()
        )
    );
}

/// Negative: [`classify_open_error`] maps any other error to `OpenFailed`.
#[test]
fn classify_open_error_generic() {
    let error = std::io::Error::other("other connection error");
    let outcome = classify_open_error(&error);
    assert_eq!(
        outcome,
        Outcome::OpenFailed("other connection error".to_owned())
    );
}

/// Negative: [`decode_canvas`] refuses JSON-RPC with bad version.
#[test]
fn decode_canvas_refused_version() {
    let bad_line =
        b"{\"jsonrpc\":\"1.0\",\"method\":\"MOUNT_SHELL_CANVAS\",\"id\":1,\"params\":{}}\n";
    let result = decode_canvas(bad_line);
    assert!(matches!(result, Err(Outcome::Refused(_))));
}

/// Negative: [`window_options`] refuses canvas failing the contract check.
#[test]
fn window_options_refuses_invalid_canvas() -> Result<(), Box<dyn std::error::Error>> {
    let mut canvas = sample_canvas()?;
    canvas.exclusive_edge = Some(Edge::Bottom); // Not in anchor (Top|Left|Right)
    assert!(matches!(
        window_options(&canvas),
        Err(LayerMapError::Contract(MountError::ExclusiveEdge))
    ));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: viewport 0x0 converts to Configured 0x0.
#[test]
fn viewport_zero_boundary() {
    let size = Size::new(px(0.0), px(0.0));
    assert_eq!(
        viewport(size),
        Outcome::Configured {
            width: 0,
            height: 0
        }
    );
}

/// Boundary: viewport with negative or NaN extents results in `OpenFailed`.
#[test]
fn viewport_negative_and_nan_boundary() {
    let neg_width = Size::new(px(-1.0), px(48.0));
    assert!(matches!(viewport(neg_width), Outcome::OpenFailed(_)));

    let nan_height = Size::new(px(1280.0), px(f32::NAN));
    assert!(matches!(viewport(nan_height), Outcome::OpenFailed(_)));
}

/// Boundary: viewport with extent at or above 2^32 results in `OpenFailed`.
#[test]
fn viewport_overflow_boundary() {
    let overflow = Size::new(px(4_294_967_296.0), px(48.0));
    assert!(matches!(viewport(overflow), Outcome::OpenFailed(_)));
}
