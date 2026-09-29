// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The shell state the inbound consumers update and the accessibility tree
//! is exported from.
//!
//! One value holds everything the tree shows: the pending decision requests,
//! the latest carbon telemetry, the managed processes, the canvas and
//! keyboard focus, and the platform preferences read on mount. Nothing is
//! cached beside it, so an export is always of the state as it is.

use thiserror::Error;

use aegis_justitia::DecisionRequest;
use aegis_tellus::CarbonTelemetry;

use crate::canvas::Canvas;
use crate::consumers::Inbound;
use crate::focus::FocusModel;
use crate::lifecycle::WindowVerdict;
use crate::processes::{ProcessTable, facility_verdict};

/// The most decision requests the shell holds pending.
pub const MAX_PENDING_DECISIONS: usize = 32;

/// The platform preferences the portal settings supply (REQ-P12-02), each
/// `false` when no preference is expressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Preferences {
    /// `org.freedesktop.appearance` `contrast` is 1: higher contrast.
    pub high_contrast: bool,
    /// `org.freedesktop.appearance` `reduced-motion` is 1: reduced motion.
    pub reduced_motion: bool,
}

/// Why the state refused an inbound message.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StateError {
    /// [`MAX_PENDING_DECISIONS`] requests are already pending.
    #[error("{MAX_PENDING_DECISIONS} decision requests are already pending")]
    DecisionsFull,
    /// A request with this request id is already pending.
    #[error("decision request {0} is already pending")]
    DuplicateDecision(String),
}

/// What an applied message changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    /// A decision request joined the pending list; its position.
    Decision(usize),
    /// The latest telemetry was replaced; the facility verdict it carries.
    Telemetry(WindowVerdict),
}

/// The whole shell model.
#[derive(Debug, Clone)]
pub struct ShellState {
    /// The managed processes.
    pub processes: ProcessTable,
    /// The canvas.
    pub canvas: Canvas,
    /// Keyboard focus and the shortcut table.
    pub focus: FocusModel,
    /// The platform preferences read on mount.
    pub preferences: Preferences,
    decisions: Vec<DecisionRequest>,
    telemetry: Option<CarbonTelemetry>,
}

impl ShellState {
    /// A shell over `canvas`, with nothing pending and no telemetry.
    #[must_use]
    pub fn new(canvas: Canvas) -> Self {
        Self {
            processes: ProcessTable::new(),
            canvas,
            focus: FocusModel::new(),
            preferences: Preferences::default(),
            decisions: Vec::new(),
            telemetry: None,
        }
    }

    /// The pending decision requests, oldest first.
    #[must_use]
    pub fn decisions(&self) -> &[DecisionRequest] {
        &self.decisions
    }

    /// The latest carbon telemetry, if any arrived.
    #[must_use]
    pub const fn telemetry(&self) -> Option<&CarbonTelemetry> {
        self.telemetry.as_ref()
    }

    /// Applies one decoded inbound message.
    ///
    /// # Errors
    ///
    /// Returns [`StateError::DecisionsFull`] past [`MAX_PENDING_DECISIONS`]
    /// and [`StateError::DuplicateDecision`] for a request id already
    /// pending. Telemetry is never refused: the latest update replaces the
    /// last, a zero-watt one included.
    pub fn apply(&mut self, inbound: Inbound) -> Result<Applied, StateError> {
        match inbound {
            Inbound::Decision(request) => self.push_decision(&request),
            Inbound::Telemetry(update) => {
                self.telemetry = Some(update);
                Ok(Applied::Telemetry(facility_verdict(&update)))
            }
        }
    }

    /// Adds a decision request to the pending list.
    fn push_decision(&mut self, request: &DecisionRequest) -> Result<Applied, StateError> {
        if self
            .decisions
            .iter()
            .any(|pending| pending.request_id == request.request_id)
        {
            return Err(StateError::DuplicateDecision(
                request.request_id.to_string(),
            ));
        }
        if self.decisions.len() >= MAX_PENDING_DECISIONS {
            return Err(StateError::DecisionsFull);
        }
        self.decisions.push(*request);
        Ok(Applied::Decision(self.decisions.len().saturating_sub(1)))
    }
}
