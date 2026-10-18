// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `MOUNT_SHELL_CANVAS`: the surface contract the P05 shell sits behind
//! (M28 E28-2, ADR-0003 decision 3, D82).
//!
//! The P05 shell asks P17 to mount its canvas as one line-delimited JSON-RPC
//! 2.0 request over `AF_UNIX` (D77), sent on the [`control`](crate::control)
//! plane. Schema `aegis.p05-p17.mount-shell-canvas.v1` is owned here, by the
//! consumer, under the rule milestone M14 set. Its parameters are the fields
//! of gpui's `LayerShellOptions` (`crates/gpui/src/platform/layer_shell.rs`
//! at zed-industries/zed `56934ec0560bce8929025497411de7f5322fb6ca`, read
//! 2026-10-18), which the shell maps its `zwlr_layer_shell_v1` surface with:
//!
//! | Member | Wire form | Protocol request (wlr-layer-shell v5) |
//! | :-- | :-- | :-- |
//! | `schema` | `"aegis.p05-p17.mount-shell-canvas.v1"` | none |
//! | `namespace` | 1 to [`MAX_NAMESPACE_BYTES`] of `[a-z0-9._-]` | `get_layer_surface` |
//! | `layer` | `"background"`, `"bottom"`, `"top"` or `"overlay"` | `get_layer_surface` |
//! | `anchor` | a list of distinct edges: `"top"`, `"bottom"`, `"left"`, `"right"` | `set_anchor` |
//! | `exclusive_zone` | optional; -1 to [`MAX_EXTENT`] | `set_exclusive_zone` |
//! | `exclusive_edge` | optional; one edge the surface is anchored to | `set_exclusive_edge` |
//! | `margin` | optional; `[top, right, bottom, left]`, each within ±[`MAX_EXTENT`] | `set_margin` |
//! | `keyboard_interactivity` | `"none"`, `"exclusive"` or `"on_demand"` | `set_keyboard_interactivity` |
//!
//! An absent optional member is gpui's `None`: the compositor's default. A
//! request that would make the compositor raise a protocol error is refused
//! here first, so the refusal does not rest on one compositor's conformance:
//! an unknown layer (`invalid_layer`), an unknown or repeated edge
//! (`invalid_anchor`), and an exclusive edge the surface is not anchored to
//! (`invalid_exclusive_edge`). The receiver answers a mounted canvas with the
//! result [`MOUNTED`] and a refusal with the JSON-RPC 2.0 error the
//! [`Refusal`](crate::Refusal) table gives it.
//!
//! The envelope rules are [`line`](mod@crate::line)'s: one line of at most
//! [`MAX_LINE_BYTES`] bytes, `jsonrpc` exactly
//! `"2.0"`, a numeric `id`, objects only, and no unknown or duplicate member.
//! Every value is `Copy` and lands in a fixed inline slot (HISS-03).
//!
//! What this does not claim: that any compositor maps the surface. Mapping it
//! is the shell's, through gpui, and a run against the host session's
//! compositor is the client half only (D79).

use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::error::{LineError, MountError};
use crate::line::{
    self, JSONRPC_VERSION, LineBuffer, MAX_LINE_BYTES, Object, Response, Token, write_line,
};

/// The one schema this build admits.
pub const MOUNT_SCHEMA_V1: &str = "aegis.p05-p17.mount-shell-canvas.v1";

/// The method: the graph relation P05 to P17.
pub const MOUNT_METHOD: &str = "MOUNT_SHELL_CANVAS";

/// The result a Response carries for a mounted canvas.
pub const MOUNTED: &str = "mounted";

/// The longest namespace, in bytes.
pub const MAX_NAMESPACE_BYTES: usize = 64;

/// The largest exclusive zone and the largest margin magnitude, in
/// surface-local pixels: the largest frame dimension this crate admits
/// ([`MAX_DIMENSION`](crate::descriptor::MAX_DIMENSION)). The protocol's
/// `int` is wider; the bound is this contract's.
pub const MAX_EXTENT: i32 = 16_384;

/// The contract versions this build admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MountSchema {
    /// `aegis.p05-p17.mount-shell-canvas.v1`.
    V1,
}

impl MountSchema {
    /// The schema's name on the wire.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::V1 => MOUNT_SCHEMA_V1,
        }
    }

    /// Parses a schema name.
    ///
    /// # Errors
    ///
    /// Returns [`MountError::Schema`] for any other name.
    pub fn parse(raw: &[u8]) -> Result<Self, MountError> {
        if raw == MOUNT_SCHEMA_V1.as_bytes() {
            Ok(Self::V1)
        } else {
            Err(MountError::Schema)
        }
    }
}

/// The layer the surface is rendered on (`zwlr_layer_shell_v1.layer`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Layer {
    /// `background`, 0.
    Background,
    /// `bottom`, 1.
    Bottom,
    /// `top`, 2.
    Top,
    /// `overlay`, 3.
    Overlay,
}

impl Layer {
    const ALL: [Self; 4] = [Self::Background, Self::Bottom, Self::Top, Self::Overlay];

    /// The name on the wire.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Bottom => "bottom",
            Self::Top => "top",
            Self::Overlay => "overlay",
        }
    }

    /// The protocol's enum value.
    #[must_use]
    pub const fn protocol_value(self) -> u32 {
        match self {
            Self::Background => 0,
            Self::Bottom => 1,
            Self::Top => 2,
            Self::Overlay => 3,
        }
    }

    /// Parses a layer name.
    ///
    /// # Errors
    ///
    /// Returns [`MountError::Layer`] for any other name.
    pub fn parse(raw: &[u8]) -> Result<Self, MountError> {
        Self::ALL
            .into_iter()
            .find(|layer| layer.name().as_bytes() == raw)
            .ok_or(MountError::Layer)
    }
}

/// One edge of the anchor rectangle (`zwlr_layer_surface_v1.anchor`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Edge {
    /// `top`, 1.
    Top,
    /// `bottom`, 2.
    Bottom,
    /// `left`, 4.
    Left,
    /// `right`, 8.
    Right,
}

impl Edge {
    const ALL: [Self; 4] = [Self::Top, Self::Bottom, Self::Left, Self::Right];

    /// The name on the wire.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    /// The protocol's bit.
    #[must_use]
    pub const fn bit(self) -> u32 {
        match self {
            Self::Top => 1,
            Self::Bottom => 2,
            Self::Left => 4,
            Self::Right => 8,
        }
    }

    fn parse(raw: &[u8]) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|edge| edge.name().as_bytes() == raw)
    }
}

/// The edges a surface is anchored to; empty centres it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Anchor(u32);

impl Anchor {
    /// Anchored to no edge.
    pub const NONE: Self = Self(0);

    /// This anchor and `edge`.
    #[must_use]
    pub const fn with(self, edge: Edge) -> Self {
        Self(self.0 | edge.bit())
    }

    /// Whether `edge` is anchored.
    #[must_use]
    pub const fn contains(self, edge: Edge) -> bool {
        self.0 & edge.bit() != 0
    }

    /// The protocol's bitfield.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }
}

impl Serialize for Anchor {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(None)?;
        for edge in Edge::ALL.into_iter().filter(|edge| self.contains(*edge)) {
            seq.serialize_element(edge.name())?;
        }
        seq.end()
    }
}

/// How keyboard input reaches the surface
/// (`zwlr_layer_surface_v1.keyboard_interactivity`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyboardInteractivity {
    /// `none`, 0.
    None,
    /// `exclusive`, 1.
    Exclusive,
    /// `on_demand`, 2 (protocol version 4).
    OnDemand,
}

impl KeyboardInteractivity {
    const ALL: [Self; 3] = [Self::None, Self::Exclusive, Self::OnDemand];

    /// The name on the wire.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Exclusive => "exclusive",
            Self::OnDemand => "on_demand",
        }
    }

    /// The protocol's enum value.
    #[must_use]
    pub const fn protocol_value(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Exclusive => 1,
            Self::OnDemand => 2,
        }
    }

    /// Parses a keyboard interactivity name.
    ///
    /// # Errors
    ///
    /// Returns [`MountError::KeyboardInteractivity`] for any other name.
    pub fn parse(raw: &[u8]) -> Result<Self, MountError> {
        Self::ALL
            .into_iter()
            .find(|mode| mode.name().as_bytes() == raw)
            .ok_or(MountError::KeyboardInteractivity)
    }
}

/// Margins between the surface and its anchors, in CSS order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Margin {
    /// Top.
    pub top: i32,
    /// Right.
    pub right: i32,
    /// Bottom.
    pub bottom: i32,
    /// Left.
    pub left: i32,
}

impl Margin {
    const fn wire(self) -> [i32; 4] {
        [self.top, self.right, self.bottom, self.left]
    }
}

/// A namespace: 1 to [`MAX_NAMESPACE_BYTES`] bytes of `[a-z0-9._-]`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Namespace {
    bytes: [u8; MAX_NAMESPACE_BYTES],
    len: usize,
}

impl Namespace {
    /// Parses a namespace.
    ///
    /// # Errors
    ///
    /// Returns [`MountError::Namespace`] for one that is empty, too long or
    /// outside its alphabet.
    pub fn parse(raw: &[u8]) -> Result<Self, MountError> {
        let admitted = |byte: &u8| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        };
        if raw.is_empty() || !raw.iter().all(admitted) {
            return Err(MountError::Namespace);
        }
        let mut namespace = Self {
            bytes: [0; MAX_NAMESPACE_BYTES],
            len: raw.len(),
        };
        namespace
            .bytes
            .get_mut(..raw.len())
            .ok_or(MountError::Namespace)?
            .copy_from_slice(raw);
        Ok(namespace)
    }

    /// The namespace.
    #[must_use]
    pub fn as_str(&self) -> &str {
        let bytes = self.bytes.get(..self.len).unwrap_or_default();
        core::str::from_utf8(bytes).unwrap_or_default()
    }
}

impl core::fmt::Debug for Namespace {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("Namespace")
            .field(&self.as_str())
            .finish()
    }
}

/// The parameters the shell maps its layer surface with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MountCanvas {
    /// The contract version.
    pub schema: MountSchema,
    /// The namespace compositors apply rules by.
    pub namespace: Namespace,
    /// The layer.
    pub layer: Layer,
    /// The anchored edges.
    pub anchor: Anchor,
    /// The exclusive zone; `None` keeps the compositor's default.
    pub exclusive_zone: Option<i32>,
    /// The edge the exclusive zone applies to; `None` lets the compositor
    /// deduce it from the anchor.
    pub exclusive_edge: Option<Edge>,
    /// The margins; `None` keeps the compositor's default.
    pub margin: Option<Margin>,
    /// How keyboard input reaches the surface.
    pub keyboard_interactivity: KeyboardInteractivity,
}

impl MountCanvas {
    /// Checks the values whose admission depends on more than their type:
    /// the exclusive zone, the exclusive edge against the anchor, and the
    /// margins.
    ///
    /// # Errors
    ///
    /// Returns [`MountError::ExclusiveZone`], [`MountError::ExclusiveEdge`]
    /// or [`MountError::Margin`].
    pub fn check(&self) -> Result<(), MountError> {
        if self
            .exclusive_zone
            .is_some_and(|zone| !(-1..=MAX_EXTENT).contains(&zone))
        {
            return Err(MountError::ExclusiveZone);
        }
        if self
            .exclusive_edge
            .is_some_and(|edge| !self.anchor.contains(edge))
        {
            return Err(MountError::ExclusiveEdge);
        }
        let out_of_range = |value: &i32| !(-MAX_EXTENT..=MAX_EXTENT).contains(value);
        if self
            .margin
            .is_some_and(|margin| margin.wire().iter().any(out_of_range))
        {
            return Err(MountError::Margin);
        }
        Ok(())
    }
}

/// A mount request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MountRequest {
    /// The request identifier the Response repeats.
    pub id: u64,
    /// The canvas parameters.
    pub canvas: MountCanvas,
}

// --- Encoding ---------------------------------------------------------------

#[derive(Serialize)]
struct CanvasOut<'a> {
    schema: &'static str,
    namespace: &'a str,
    layer: &'static str,
    anchor: Anchor,
    #[serde(skip_serializing_if = "Option::is_none")]
    exclusive_zone: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exclusive_edge: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    margin: Option<[i32; 4]>,
    keyboard_interactivity: &'static str,
}

#[derive(Serialize)]
struct RequestOut<'a> {
    jsonrpc: &'static str,
    method: &'static str,
    params: CanvasOut<'a>,
    id: u64,
}

/// Encodes `request` as one line into `buffer`, after checking its canvas,
/// so a sender cannot put a line on the wire the receiver would refuse for
/// its values.
///
/// # Errors
///
/// Returns [`LineError::Mount`] for a canvas [`MountCanvas::check`] refuses
/// and [`LineError::TooLong`] when the line would exceed the bound.
pub fn encode_request<'b>(
    request: &MountRequest,
    buffer: &'b mut LineBuffer,
) -> Result<&'b [u8], LineError> {
    let canvas = &request.canvas;
    canvas.check()?;
    let out = RequestOut {
        jsonrpc: JSONRPC_VERSION,
        method: MOUNT_METHOD,
        params: CanvasOut {
            schema: canvas.schema.name(),
            namespace: canvas.namespace.as_str(),
            layer: canvas.layer.name(),
            anchor: canvas.anchor,
            exclusive_zone: canvas.exclusive_zone,
            exclusive_edge: canvas.exclusive_edge.map(Edge::name),
            margin: canvas.margin.map(Margin::wire),
            keyboard_interactivity: canvas.keyboard_interactivity.name(),
        },
        id: request.id,
    };
    write_line(&out, buffer)
}

/// Encodes `response` as one line into `buffer`, with [`MOUNTED`] as the
/// result of an admitted request.
///
/// # Errors
///
/// Returns [`LineError::TooLong`] when the line would exceed the bound,
/// which a Response cannot reach.
pub fn encode_response<'b>(
    response: &Response,
    buffer: &'b mut LineBuffer,
) -> Result<&'b [u8], LineError> {
    line::encode_response_as(response, MOUNTED, buffer)
}

// --- Decoding ---------------------------------------------------------------

/// The anchor list as read: the edges, and whether any was unknown or
/// repeated.
#[derive(Clone, Copy)]
struct EdgesIn {
    anchor: Anchor,
    refused: bool,
}

struct EdgesVisitor;

impl<'de> de::Visitor<'de> for EdgesVisitor {
    type Value = EdgesIn;

    fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("a list of edges")
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<EdgesIn, A::Error> {
        let mut edges = EdgesIn {
            anchor: Anchor::NONE,
            refused: false,
        };
        // A line of MAX_LINE_BYTES cannot list more elements than bytes.
        for _ in 0..=MAX_LINE_BYTES {
            let Some(token) = seq.next_element::<Token<8>>()? else {
                return Ok(edges);
            };
            match Edge::parse(token.as_bytes()) {
                Some(edge) if !edges.anchor.contains(edge) => {
                    edges.anchor = edges.anchor.with(edge);
                }
                _ => edges.refused = true,
            }
        }
        Err(de::Error::custom("the anchor list exceeds the line bound"))
    }
}

impl<'de> Deserialize<'de> for EdgesIn {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(EdgesVisitor)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CanvasIn {
    schema: Token<48>,
    namespace: Token<MAX_NAMESPACE_BYTES>,
    layer: Token<16>,
    anchor: EdgesIn,
    #[serde(default)]
    exclusive_zone: Option<i64>,
    #[serde(default)]
    exclusive_edge: Option<Token<8>>,
    #[serde(default)]
    margin: Option<[i64; 4]>,
    keyboard_interactivity: Token<16>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestIn {
    #[serde(default)]
    jsonrpc: Option<Token<8>>,
    #[serde(default)]
    method: Option<Token<32>>,
    #[serde(default)]
    params: Option<Object<CanvasIn>>,
    #[serde(default)]
    id: Option<u64>,
}

/// A wire integer narrowed to `i32`; one that does not fit is refused as
/// `error`.
fn narrow(value: i64, error: MountError) -> Result<i32, MountError> {
    i32::try_from(value).map_err(|_| error)
}

impl CanvasIn {
    fn into_canvas(self) -> Result<MountCanvas, MountError> {
        let schema = MountSchema::parse(self.schema.as_bytes())?;
        let namespace = Namespace::parse(self.namespace.as_bytes())?;
        let layer = Layer::parse(self.layer.as_bytes())?;
        if self.anchor.refused {
            return Err(MountError::Anchor);
        }
        let exclusive_zone = self
            .exclusive_zone
            .map(|zone| narrow(zone, MountError::ExclusiveZone))
            .transpose()?;
        let exclusive_edge = self
            .exclusive_edge
            .map(|token| Edge::parse(token.as_bytes()).ok_or(MountError::ExclusiveEdge))
            .transpose()?;
        let margin = match self.margin {
            Some([top, right, bottom, left]) => Some(Margin {
                top: narrow(top, MountError::Margin)?,
                right: narrow(right, MountError::Margin)?,
                bottom: narrow(bottom, MountError::Margin)?,
                left: narrow(left, MountError::Margin)?,
            }),
            None => None,
        };
        let canvas = MountCanvas {
            schema,
            namespace,
            layer,
            anchor: self.anchor.anchor,
            exclusive_zone,
            exclusive_edge,
            margin,
            keyboard_interactivity: KeyboardInteractivity::parse(
                self.keyboard_interactivity.as_bytes(),
            )?,
        };
        canvas.check()?;
        Ok(canvas)
    }
}

/// Decodes one mount request line.
///
/// # Errors
///
/// Returns the [`LineError`] naming why the line was refused: the envelope's
/// errors as [`line::decode_request`] reports them, with
/// [`LineError::Method`] for any method but [`MOUNT_METHOD`], and
/// [`LineError::Mount`] for parameters the contract refuses.
pub fn decode_request(message: &[u8]) -> Result<MountRequest, LineError> {
    let text = line::body(message)?;
    let Object(raw): Object<RequestIn> =
        serde_json::from_slice(text).map_err(|error| LineError::from(&error))?;
    if !raw.jsonrpc.is_some_and(|token| token.is(JSONRPC_VERSION)) {
        return Err(LineError::Version);
    }
    if !raw.method.is_some_and(|token| token.is(MOUNT_METHOD)) {
        return Err(LineError::Method);
    }
    let id = raw.id.ok_or(LineError::Id)?;
    let Object(params) = raw.params.ok_or(LineError::Params)?;
    Ok(MountRequest {
        id,
        canvas: params.into_canvas()?,
    })
}

/// Decodes one mount Response line.
///
/// # Errors
///
/// Returns [`LineError::Version`] for another protocol version and
/// [`LineError::Response`] for a line with both, neither or an unknown
/// outcome, [`crate::line::ATTACHED`] included.
pub fn decode_response(message: &[u8]) -> Result<Response, LineError> {
    line::decode_response_as(message, MOUNTED)
}
