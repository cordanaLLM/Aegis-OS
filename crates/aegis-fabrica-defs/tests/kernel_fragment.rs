// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The tracked kernel configuration fragment, tied to its producer.
//!
//! Milestone M26 applied `build/kernel/50-aegis-requirement.config` to a named
//! base configuration and built the result. D112 retired that build: the
//! kernel the gates boot is the one Nucleus publishes against
//! `build/kernel-requirement.json`. The fragment stays the Kconfig form of the
//! requirement, and it is only a legitimate statement of it if it is what
//! [`KernelRequirement::config_fragment`] renders from the payload; a
//! hand-edited copy would agree with a file instead of with the schema. These
//! tests are the binding.

mod common;

use std::collections::BTreeMap;

use aegis_fabrica_defs::KernelRequirement;
use aegis_fabrica_defs::kernel::ConfigState;

use common::{Fallible, reviewed, reviewed_payload};

/// Substrings that must never appear in a reviewed file.
const PRIVATE_MARKERS: [&str; 3] = ["/home/", ".workingdir", "notebook-prepared"];

/// The reviewed product kernel requirement, as this repository ships it.
fn shipped() -> Result<KernelRequirement, Box<dyn std::error::Error>> {
    Ok(KernelRequirement::decode(&reviewed_payload(
        "kernel-requirement.json",
    )?)?)
}

/// The tracked fragment M26 applies, read from `build/kernel/`.
fn tracked_fragment() -> Result<String, Box<dyn std::error::Error>> {
    Ok(reviewed("kernel/50-aegis-requirement.config")?.1)
}

/// The tracked base-support fragment M26 applies before the requirement one.
fn tracked_support() -> Result<String, Box<dyn std::error::Error>> {
    Ok(reviewed("kernel/10-base-support.config")?.1)
}

/// Every `CONFIG_X=...` assignment in a fragment, as symbol names.
fn assigned_symbols(fragment: &str) -> Vec<String> {
    fragment
        .lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(symbol, _)| symbol.starts_with("CONFIG_"))
        .map(|(symbol, _)| symbol.to_owned())
        .collect()
}

/// A fragment read the way a checker reads a produced configuration.
///
/// This is what [`RequiredState::satisfied_by`] is given: a symbol the
/// fragment assigns maps to its state, and a symbol it says nothing about is
/// simply absent from the map, which the checker refuses rather than defaults.
fn observed_states(fragment: &str) -> BTreeMap<String, ConfigState> {
    let mut states = BTreeMap::new();
    for line in fragment.lines() {
        if let Some(symbol) = line
            .strip_prefix("# ")
            .and_then(|rest| rest.strip_suffix(" is not set"))
        {
            states.insert(symbol.to_owned(), ConfigState::NotSet);
        } else if let Some((symbol, value)) = line.split_once('=')
            && let Some(state) = ConfigState::parse(value)
        {
            states.insert(symbol.to_owned(), state);
        }
    }
    states
}

/// The payload rows the given observation does not satisfy, by symbol.
fn unmet_symbols(
    requirement: &KernelRequirement,
    states: &BTreeMap<String, ConfigState>,
) -> Vec<String> {
    requirement
        .features
        .iter()
        .map(|feature| (feature.symbol.to_string(), feature.state))
        .filter(|(symbol, state)| !state.satisfied_by(states.get(symbol).copied()))
        .map(|(symbol, _)| symbol)
        .collect()
}

// --- Positive -------------------------------------------------------------

/// Positive: the tracked fragment is exactly what the payload renders.
///
/// Byte-identical, not merely equivalent. A fragment that carried the same
/// assignments in another order, or without the requirement identifier above
/// each one, would still build a conforming kernel but would no longer be
/// evidence that the M18 schema is the source.
#[test]
fn the_tracked_fragment_is_byte_identical_to_what_the_payload_renders() -> Fallible {
    assert_eq!(tracked_fragment()?, shipped()?.config_fragment()?);
    Ok(())
}

/// Positive: the fragment carries one assignment per feature row, and no other.
#[test]
fn the_tracked_fragment_assigns_exactly_the_payload_symbols() -> Fallible {
    let requirement = shipped()?;
    let mut expected: Vec<String> = requirement
        .features
        .iter()
        .map(|feature| feature.symbol.to_string())
        .collect();
    let mut found = assigned_symbols(&tracked_fragment()?);
    expected.sort();
    found.sort();
    assert_eq!(found, expected);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: the base-support fragment states no product requirement.
///
/// The two fragments are applied in order and the support one is hand written.
/// If it assigned a symbol the payload demands, the build could satisfy a
/// requirement row from a hand-written file while the schema-derived fragment
/// said nothing, and the read-back would still pass. The separation is what
/// keeps the requirement traceable to `build/kernel-requirement.json`.
#[test]
fn the_base_support_fragment_assigns_no_symbol_the_payload_requires() -> Fallible {
    let required: Vec<String> = shipped()?
        .features
        .iter()
        .map(|feature| feature.symbol.to_string())
        .collect();
    for symbol in assigned_symbols(&tracked_support()?) {
        assert!(
            !required.contains(&symbol),
            "{symbol} is a requirement row and must not be set by the support fragment"
        );
    }
    Ok(())
}

/// Negative: a fragment with one line dropped leaves that requirement unmet.
///
/// The check has to notice a subtraction, because that is the shape a silent
/// relaxation takes: the build still succeeds and the missing option is simply
/// never asked for. Asserting that a thinned string differs from the one it
/// was thinned from would prove only that removing a line changes a string, so
/// both fragments go through the checker the requirement is actually enforced
/// with, [`RequiredState::satisfied_by`]: the whole fragment leaves no row
/// unmet, and the thinned one leaves exactly the dropped symbol unmet, because
/// an unrecorded symbol never satisfies a row.
#[test]
fn a_fragment_with_a_dropped_assignment_leaves_that_requirement_unmet() -> Fallible {
    let requirement = shipped()?;
    let fragment = tracked_fragment()?;
    let dropped = "CONFIG_PREEMPT_RT";
    let assignment = format!("{dropped}=");
    let mut thinned = String::new();
    for line in fragment
        .lines()
        .filter(|line| !line.starts_with(&assignment))
    {
        thinned.push_str(line);
        thinned.push('\n');
    }

    let whole = observed_states(&fragment);
    let thinned_states = observed_states(&thinned);
    assert!(whole.contains_key(dropped));
    assert!(!thinned_states.contains_key(dropped));
    assert_eq!(thinned_states.len(), whole.len() - 1);

    assert_eq!(unmet_symbols(&requirement, &whole), Vec::<String>::new());
    assert_eq!(
        unmet_symbols(&requirement, &thinned_states),
        vec![dropped.to_owned()]
    );
    assert_ne!(thinned, requirement.config_fragment()?);
    Ok(())
}

/// Negative: no reviewed kernel input names a private path.
#[test]
fn no_reviewed_kernel_input_names_a_private_path() -> Fallible {
    let sources = [
        tracked_fragment()?,
        tracked_support()?,
        reviewed("kernel/nucleus-artifact.pin.json")?.1,
    ];
    for text in &sources {
        for marker in PRIVATE_MARKERS {
            assert!(!text.contains(marker), "a reviewed input names {marker}");
        }
    }
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the pinned Nucleus release is at or above the payload's ABI floor.
///
/// The kernel the latency fixture and the M10 gate boot is the one
/// `build/kernel/nucleus-artifact.pin.json` pins (D112). A release older than
/// the minimum the payload declares could not satisfy it whatever its
/// configuration said, so the pin and the payload have to agree before any
/// boot is attempted.
#[test]
fn the_pinned_nucleus_release_is_not_below_the_payload_abi_floor() -> Fallible {
    let pin = reviewed("kernel/nucleus-artifact.pin.json")?.1;
    let document: serde_json::Value = serde_json::from_str(&pin)?;
    let pinned = document
        .pointer("/kernel/release")
        .and_then(serde_json::Value::as_str)
        .ok_or("the pin records no kernel release")?;
    let requirement = shipped()?;
    let minimum = requirement.abi.minimum_release.to_string();
    let parse = |text: &str| -> Vec<u32> {
        text.split(['.', '-'])
            .map_while(|part| part.parse::<u32>().ok())
            .collect()
    };
    assert!(
        parse(pinned) >= parse(&minimum),
        "pinned {pinned} is below the payload floor {minimum}"
    );
    // The comparison admits the floor itself and refuses one step below it.
    let floor = parse(&minimum);
    let mut below = floor.clone();
    let last = below
        .last_mut()
        .ok_or("the payload floor has no numeric part")?;
    *last = last
        .checked_sub(1)
        .ok_or("the payload floor ends in zero")?;
    assert!(floor >= parse(&minimum));
    assert!(below < floor);
    Ok(())
}
