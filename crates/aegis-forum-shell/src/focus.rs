// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The keyboard model (REQ-P05-09, REQ-P05-10): shell regions, focus inside
//! the canvas, instrument nesting and single-character shortcuts.
//!
//! | Key | In a region | On a canvas node | In an instrument level |
//! | :-- | :-- | :-- | :-- |
//! | Tab, Shift+Tab | next, previous region | leaves the canvas | leaves the canvas, unless the level swallows it |
//! | Next, Previous | -- | neighbour in focus order, camera follows | -- |
//! | Enter | -- | opens the instrument | opens the next nested level |
//! | Escape | -- | -- | back to the owner: the outer level, or the node |
//! | Space, Control+key | -- | the node's bound action | the level's bound action |
//! | a letter | -- | a single-character shortcut, if enabled | -- |
//!
//! Entering the canvas lands on the node focused last, else the first one, so
//! Tab and Shift+Tab move between regions and never through a thousand nodes.
//! A level that swallows Tab is modelled, not forbidden, so the walk in
//! [`crate::walk`] can show that it catches one.

use thiserror::Error;

use crate::canvas::registry::{ActionKind, Binding, NodeKey, TabPolicy};
use crate::canvas::{Canvas, CanvasError, Target};

/// The shell regions, in Tab order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    /// The status bar: carbon telemetry and the tray.
    StatusBar,
    /// The spatial canvas.
    Canvas,
    /// The pending decision requests.
    Decisions,
}

impl Region {
    /// The regions, in Tab order.
    pub const ORDER: [Self; 3] = [Self::StatusBar, Self::Canvas, Self::Decisions];

    /// The region Tab moves to.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::StatusBar => Self::Canvas,
            Self::Canvas => Self::Decisions,
            Self::Decisions => Self::StatusBar,
        }
    }

    /// The region Shift+Tab moves to.
    #[must_use]
    pub const fn previous(self) -> Self {
        match self {
            Self::StatusBar => Self::Decisions,
            Self::Canvas => Self::StatusBar,
            Self::Decisions => Self::Canvas,
        }
    }
}

/// Where keyboard focus is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// A region as a whole: the status bar, the decisions, or an empty
    /// canvas.
    Region(Region),
    /// A canvas node or one of its instrument levels.
    Target(Target),
}

/// A key press the model understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Tab.
    Tab,
    /// Shift+Tab.
    ShiftTab,
    /// The next node in focus order (an arrow key).
    Next,
    /// The previous node in focus order (an arrow key).
    Previous,
    /// Enter.
    Enter,
    /// Escape.
    Escape,
    /// The space bar.
    Space,
    /// Control and a character.
    Ctrl(char),
    /// A character with no modifier.
    Char(char),
}

/// A global command a single-character shortcut can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Fit every node into the viewport.
    FitView,
    /// Double the camera scale.
    ZoomIn,
    /// Halve the camera scale.
    ZoomOut,
}

/// What one key press did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOutcome {
    /// Focus moved.
    Moved(Focus),
    /// An action ran on a target.
    Invoked(Target, ActionKind),
    /// A shortcut ran a command.
    Command(Command),
    /// Nothing happened.
    Ignored,
}

/// Why a key press or a shortcut change was refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FocusError {
    /// The canvas refused an operation.
    #[error(transparent)]
    Canvas(#[from] CanvasError),
    /// A shortcut letter is not one ASCII letter, or another command has it.
    #[error("{letter:?} cannot be a shortcut: it must be one free ASCII letter")]
    Letter {
        /// The letter asked for.
        letter: char,
    },
}

/// The single-character shortcuts, each of which can be turned off or
/// remapped (REQ-P05-09, WCAG 2.2 SC 2.1.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortcuts {
    entries: [(char, Command, bool); 3],
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            entries: [
                ('f', Command::FitView, true),
                ('i', Command::ZoomIn, true),
                ('o', Command::ZoomOut, true),
            ],
        }
    }
}

impl Shortcuts {
    /// The command an enabled shortcut on `letter` runs.
    #[must_use]
    pub fn lookup(&self, letter: char) -> Option<Command> {
        self.entries
            .iter()
            .find(|(bound, _, enabled)| *enabled && *bound == letter)
            .map(|(_, command, _)| *command)
    }

    /// The letter `command` is bound to, and whether it is enabled.
    #[must_use]
    pub fn binding(&self, command: Command) -> Option<(char, bool)> {
        self.entries
            .iter()
            .find(|(_, bound, _)| *bound == command)
            .map(|(letter, _, enabled)| (*letter, *enabled))
    }

    /// Turns `command`'s shortcut on or off.
    pub fn set_enabled(&mut self, command: Command, enabled: bool) {
        for entry in &mut self.entries {
            if entry.1 == command {
                entry.2 = enabled;
            }
        }
    }

    /// Moves `command`'s shortcut to `letter`.
    ///
    /// # Errors
    ///
    /// Returns [`FocusError::Letter`] unless `letter` is an ASCII letter no
    /// other command is bound to.
    pub fn remap(&mut self, command: Command, letter: char) -> Result<(), FocusError> {
        let taken = self
            .entries
            .iter()
            .any(|(bound, other, _)| *bound == letter && *other != command);
        if !letter.is_ascii_alphabetic() || taken {
            return Err(FocusError::Letter { letter });
        }
        for entry in &mut self.entries {
            if entry.1 == command {
                entry.0 = letter;
            }
        }
        Ok(())
    }
}

/// Keyboard focus and the shortcut table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusModel {
    focus: Focus,
    last: Option<NodeKey>,
    shortcuts: Shortcuts,
}

impl Default for FocusModel {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusModel {
    /// Focus on the status bar, no node focused yet, default shortcuts.
    #[must_use]
    pub fn new() -> Self {
        Self {
            focus: Focus::Region(Region::StatusBar),
            last: None,
            shortcuts: Shortcuts::default(),
        }
    }

    /// Where focus is.
    #[must_use]
    pub const fn focus(&self) -> Focus {
        self.focus
    }

    /// The shortcut table.
    #[must_use]
    pub const fn shortcuts(&self) -> &Shortcuts {
        &self.shortcuts
    }

    /// The shortcut table, mutably.
    pub const fn shortcuts_mut(&mut self) -> &mut Shortcuts {
        &mut self.shortcuts
    }

    /// Focuses the node under `key` directly, as an assistive technology's
    /// focus request would, and moves the camera until it is in view
    /// (REQ-P05-09). Returns whether the camera moved.
    ///
    /// # Errors
    ///
    /// Propagates [`Canvas::reveal`].
    pub fn focus_node(&mut self, canvas: &mut Canvas, key: NodeKey) -> Result<bool, FocusError> {
        let moved = canvas.reveal(key)?;
        self.focus = Focus::Target(Target::Node(key));
        self.last = Some(key);
        Ok(moved)
    }

    /// Handles one key press.
    ///
    /// # Errors
    ///
    /// Propagates a canvas refusal; an unhandled key is
    /// [`KeyOutcome::Ignored`], not an error.
    pub fn press(&mut self, canvas: &mut Canvas, key: Key) -> Result<KeyOutcome, FocusError> {
        match self.focus {
            Focus::Region(region) => self.press_region(canvas, region, key),
            Focus::Target(Target::Node(node)) => self.press_node(canvas, node, key),
            Focus::Target(Target::Level(node, depth)) => self.press_level(canvas, node, depth, key),
        }
    }

    /// Moves focus into `region`; the canvas lands on its last node.
    fn enter(&mut self, canvas: &mut Canvas, region: Region) -> Result<KeyOutcome, FocusError> {
        if region != Region::Canvas {
            self.focus = Focus::Region(region);
            return Ok(KeyOutcome::Moved(self.focus));
        }
        let registry = canvas.registry();
        let landing = self
            .last
            .filter(|key| registry.get(*key).is_some())
            .or_else(|| registry.at(0).map(|node| node.key));
        match landing {
            Some(key) => {
                self.focus_node(canvas, key)?;
            }
            None => self.focus = Focus::Region(Region::Canvas),
        }
        Ok(KeyOutcome::Moved(self.focus))
    }

    /// A key while a region as a whole has focus.
    fn press_region(
        &mut self,
        canvas: &mut Canvas,
        region: Region,
        key: Key,
    ) -> Result<KeyOutcome, FocusError> {
        match key {
            Key::Tab => self.enter(canvas, region.next()),
            Key::ShiftTab => self.enter(canvas, region.previous()),
            _ => Ok(KeyOutcome::Ignored),
        }
    }

    /// A key while a canvas node has focus.
    fn press_node(
        &mut self,
        canvas: &mut Canvas,
        node: NodeKey,
        key: Key,
    ) -> Result<KeyOutcome, FocusError> {
        match key {
            Key::Tab => self.enter(canvas, Region::Canvas.next()),
            Key::ShiftTab => self.enter(canvas, Region::Canvas.previous()),
            Key::Next | Key::Previous => self.step(canvas, node, key == Key::Next),
            Key::Enter => Ok(self.open(canvas, node, 0)),
            Key::Space | Key::Ctrl(_) => invoke(canvas, Target::Node(node), key),
            Key::Char(letter) => Ok(self.shortcut(canvas, letter)),
            Key::Escape => Ok(KeyOutcome::Ignored),
        }
    }

    /// A key while an instrument level has focus.
    fn press_level(
        &mut self,
        canvas: &mut Canvas,
        node: NodeKey,
        depth: usize,
        key: Key,
    ) -> Result<KeyOutcome, FocusError> {
        match key {
            Key::Escape => {
                self.focus = Focus::Target(match depth.checked_sub(1) {
                    Some(outer) => Target::Level(node, outer),
                    None => Target::Node(node),
                });
                Ok(KeyOutcome::Moved(self.focus))
            }
            Key::Enter => Ok(self.open(canvas, node, depth.saturating_add(1))),
            Key::Tab | Key::ShiftTab => self.leave_level(canvas, node, depth, key),
            Key::Space | Key::Ctrl(_) => invoke(canvas, Target::Level(node, depth), key),
            Key::Next | Key::Previous | Key::Char(_) => Ok(KeyOutcome::Ignored),
        }
    }

    /// Tab or Shift+Tab from an instrument level: out of the canvas, unless
    /// the level swallows it.
    fn leave_level(
        &mut self,
        canvas: &mut Canvas,
        node: NodeKey,
        depth: usize,
        key: Key,
    ) -> Result<KeyOutcome, FocusError> {
        let policy = canvas
            .registry()
            .get(node)
            .and_then(|found| found.levels.get(depth))
            .map(|level| level.tab);
        if policy != Some(TabPolicy::Leaves) {
            return Ok(KeyOutcome::Ignored);
        }
        let region = if key == Key::Tab {
            Region::Canvas.next()
        } else {
            Region::Canvas.previous()
        };
        self.enter(canvas, region)
    }

    /// Moves to the neighbouring node in focus order, if there is one.
    fn step(
        &mut self,
        canvas: &mut Canvas,
        node: NodeKey,
        forward: bool,
    ) -> Result<KeyOutcome, FocusError> {
        let registry = canvas.registry();
        let neighbour = registry.position(node).and_then(|position| {
            let target = if forward {
                position.checked_add(1)
            } else {
                position.checked_sub(1)
            };
            target
                .and_then(|index| registry.at(index))
                .map(|found| found.key)
        });
        let Some(neighbour) = neighbour else {
            return Ok(KeyOutcome::Ignored);
        };
        self.focus_node(canvas, neighbour)?;
        Ok(KeyOutcome::Moved(self.focus))
    }

    /// Opens instrument level `depth` of `node`, if the node has it.
    fn open(&mut self, canvas: &Canvas, node: NodeKey, depth: usize) -> KeyOutcome {
        let exists = canvas
            .registry()
            .get(node)
            .is_some_and(|found| found.levels.get(depth).is_some());
        if !exists {
            return KeyOutcome::Ignored;
        }
        self.focus = Focus::Target(Target::Level(node, depth));
        KeyOutcome::Moved(self.focus)
    }

    /// Runs the command an enabled shortcut on `letter` names, if any.
    fn shortcut(&self, canvas: &mut Canvas, letter: char) -> KeyOutcome {
        let Some(command) = self.shortcuts.lookup(letter) else {
            return KeyOutcome::Ignored;
        };
        match command {
            Command::FitView => {
                if let Some(bounds) = canvas.content_bounds() {
                    canvas.camera_mut().fit(&bounds);
                }
            }
            Command::ZoomIn => canvas.camera_mut().zoom_by(2.0),
            Command::ZoomOut => canvas.camera_mut().zoom_by(0.5),
        }
        KeyOutcome::Command(command)
    }
}

/// Runs the action on `target` whose keyboard binding is `key`, if any.
fn invoke(canvas: &mut Canvas, target: Target, key: Key) -> Result<KeyOutcome, FocusError> {
    let binding = match key {
        Key::Space => Binding::Space,
        Key::Ctrl(letter) => Binding::Ctrl(letter),
        _ => return Ok(KeyOutcome::Ignored),
    };
    let registry = canvas.registry();
    let actions = match target {
        Target::Node(node) => registry.get(node).map(|found| found.actions.as_slice()),
        Target::Level(node, depth) => registry
            .get(node)
            .and_then(|found| found.levels.get(depth))
            .map(|level| level.actions.as_slice()),
    };
    let kind = actions
        .unwrap_or_default()
        .iter()
        .find(|action| action.keyboard == Some(binding))
        .map(|action| action.kind);
    let Some(kind) = kind else {
        return Ok(KeyOutcome::Ignored);
    };
    canvas.apply(target, kind)?;
    Ok(KeyOutcome::Invoked(target, kind))
}
