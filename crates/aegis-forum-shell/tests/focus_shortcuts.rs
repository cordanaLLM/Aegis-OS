// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The single-character shortcuts: `Shortcuts::lookup` and
//! `FocusModel::shortcuts` (REQ-P05-09, WCAG 2.2 SC 2.1.4).
//!
//! The defaults, as the source documents them: `f` fits the view, `i` zooms
//! in, `o` zooms out.
//!
//! Positive: each default letter runs its command, a remapped letter runs the
//! command, the model exposes the table it presses through. Negative: an
//! unbound, upper-case or disabled letter runs nothing, a taken or non-letter
//! remap is refused and leaves the table as it was. Boundary: remapping to
//! the command's own letter, to an upper-case letter, and toggling a
//! shortcut off and on.

mod common;

use aegis_forum_shell::focus::{Command, FocusError, FocusModel, Shortcuts};

use common::Fallible;

// --- Positive -------------------------------------------------------------

/// Positive: the three default letters map to their commands.
#[test]
fn the_default_letters_run_their_commands() {
    let shortcuts = Shortcuts::default();
    assert_eq!(shortcuts.lookup('f'), Some(Command::FitView));
    assert_eq!(shortcuts.lookup('i'), Some(Command::ZoomIn));
    assert_eq!(shortcuts.lookup('o'), Some(Command::ZoomOut));
}

/// Positive: after a remap the new letter runs the command and the old one
/// runs nothing.
#[test]
fn a_remapped_letter_runs_the_command_and_the_old_one_does_not() -> Fallible {
    let mut shortcuts = Shortcuts::default();
    shortcuts.remap(Command::ZoomIn, 'z')?;
    assert_eq!(shortcuts.lookup('z'), Some(Command::ZoomIn));
    assert_eq!(shortcuts.lookup('i'), None);
    assert_eq!(shortcuts.lookup('f'), Some(Command::FitView));
    Ok(())
}

/// Positive: a new model exposes the default table, and a change made through
/// `shortcuts_mut` shows in `shortcuts`.
#[test]
fn the_model_exposes_the_table_it_presses_through() -> Fallible {
    let mut model = FocusModel::new();
    assert_eq!(model.shortcuts(), &Shortcuts::default());
    assert_eq!(model.shortcuts().lookup('o'), Some(Command::ZoomOut));
    model.shortcuts_mut().remap(Command::ZoomOut, 'q')?;
    model.shortcuts_mut().set_enabled(Command::FitView, false);
    assert_eq!(model.shortcuts().lookup('q'), Some(Command::ZoomOut));
    assert_eq!(model.shortcuts().lookup('f'), None);
    assert_eq!(
        model.shortcuts().binding(Command::FitView),
        Some(('f', false))
    );
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a letter nobody is bound to, an upper-case letter, a digit and
/// the NUL character run nothing.
#[test]
fn an_unbound_letter_runs_nothing() {
    let shortcuts = Shortcuts::default();
    for letter in ['x', 'F', 'I', 'O', '1', '\0', ' '] {
        assert_eq!(shortcuts.lookup(letter), None, "{letter:?}");
    }
}

/// Negative: a disabled shortcut runs nothing; the others still do.
#[test]
fn a_disabled_shortcut_runs_nothing() {
    let mut shortcuts = Shortcuts::default();
    shortcuts.set_enabled(Command::ZoomIn, false);
    assert_eq!(shortcuts.lookup('i'), None);
    assert_eq!(shortcuts.lookup('o'), Some(Command::ZoomOut));
    assert_eq!(shortcuts.lookup('f'), Some(Command::FitView));
}

/// Negative: a letter another command holds, a digit and a non-ASCII letter
/// are refused with `Letter`, and the table, read through the model, is
/// unchanged.
#[test]
fn a_refused_remap_leaves_the_table_as_it_was() {
    let mut model = FocusModel::new();
    for letter in ['o', '7', 'é'] {
        let result = model.shortcuts_mut().remap(Command::FitView, letter);
        assert_eq!(result, Err(FocusError::Letter { letter }));
    }
    assert_eq!(model.shortcuts(), &Shortcuts::default());
}

// --- Boundary -------------------------------------------------------------

/// Boundary: remapping a command to its own letter is admitted and changes
/// nothing.
#[test]
fn remapping_to_the_own_letter_changes_nothing() -> Fallible {
    let mut shortcuts = Shortcuts::default();
    shortcuts.remap(Command::FitView, 'f')?;
    assert_eq!(shortcuts, Shortcuts::default());
    assert_eq!(shortcuts.lookup('f'), Some(Command::FitView));
    Ok(())
}

/// Boundary: an upper-case ASCII letter is a letter: `F` runs the command and
/// `f` no longer does (lookup is case-sensitive).
#[test]
fn an_upper_case_letter_is_a_distinct_shortcut() -> Fallible {
    let mut shortcuts = Shortcuts::default();
    shortcuts.remap(Command::FitView, 'F')?;
    assert_eq!(shortcuts.lookup('F'), Some(Command::FitView));
    assert_eq!(shortcuts.lookup('f'), None);
    Ok(())
}

/// Boundary: a shortcut turned off and on again runs its command, and the
/// disabled state keeps the letter bound: its binding reads `(letter, false)`
/// then `(letter, true)`.
#[test]
fn a_shortcut_turned_off_and_on_runs_again() {
    let mut shortcuts = Shortcuts::default();
    shortcuts.set_enabled(Command::ZoomOut, false);
    assert_eq!(shortcuts.binding(Command::ZoomOut), Some(('o', false)));
    assert_eq!(shortcuts.lookup('o'), None);
    shortcuts.set_enabled(Command::ZoomOut, true);
    assert_eq!(shortcuts.binding(Command::ZoomOut), Some(('o', true)));
    assert_eq!(shortcuts.lookup('o'), Some(Command::ZoomOut));
}
