// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Triples for the drop-in reader: syntax, repeats, bounds and the round trip.

mod common;

use aegis_fabrica_defs::{
    CONTRACT_VERSION, DefinitionError, Entry, MAX_ENTRIES, MAX_LINE_BYTES, MAX_LINES, MAX_SECTIONS,
    UnitFile,
};

use common::Fallible;

const SAMPLE: &str = "# a comment\n[Partition]\nType=esp\n\nLabel=ESP\n";

// --- Positive -------------------------------------------------------------

/// Positive: a drop-in yields its section, its assignments and their lines.
#[test]
fn a_drop_in_yields_its_sections_and_assignments() -> Fallible {
    let unit = UnitFile::parse("00-esp.conf", SAMPLE)?;
    assert_eq!(unit.name(), "00-esp.conf");
    assert_eq!(unit.sections(), ["Partition"]);
    assert_eq!(unit.value("Partition", "Type"), Some("esp"));
    assert_eq!(unit.value("Partition", "Label"), Some("ESP"));
    assert_eq!(unit.line_of("Partition", "Type"), Some(3));
    assert_eq!(unit.line_of("Partition", "Label"), Some(5));
    let expected = Entry {
        section: "Partition".to_owned(),
        key: "Type".to_owned(),
        value: "esp".to_owned(),
        line: 3,
    };
    assert_eq!(unit.entries().first(), Some(&expected));
    assert_eq!(unit.entries().len(), 2);
    Ok(())
}

/// Positive: the canonical rendering parses back to the same declarations.
///
/// The rendering drops comments, so the re-read assignments sit on different
/// lines. That is the point of the canonical form: a round trip preserves what
/// the file declares, and [`Entry::line`] records where the text said it, not
/// what it said. Rendering is therefore idempotent from the second pass on.
#[test]
fn a_rendered_drop_in_round_trips() -> Fallible {
    let unit = UnitFile::parse("00-esp.conf", SAMPLE)?;
    let again = UnitFile::parse("00-esp.conf", &unit.render())?;
    assert_eq!(unit.render(), "[Partition]\nType=esp\nLabel=ESP\n\n");
    assert_eq!(again.render(), unit.render());
    assert_eq!(again.sections(), unit.sections());
    let declared: Vec<(&str, &str)> = unit
        .entries()
        .iter()
        .map(|entry| (entry.key.as_str(), entry.value.as_str()))
        .collect();
    let reread: Vec<(&str, &str)> = again
        .entries()
        .iter()
        .map(|entry| (entry.key.as_str(), entry.value.as_str()))
        .collect();
    assert_eq!(reread, declared);
    let third = UnitFile::parse("00-esp.conf", &again.render())?;
    assert_eq!(third, again);
    Ok(())
}

/// Positive: comments and blank lines declare nothing.
#[test]
fn comments_and_blank_lines_declare_nothing() -> Fallible {
    let unit = UnitFile::parse("x.conf", "# one\n; two\n\n[Partition]\n")?;
    assert!(unit.entries().is_empty());
    assert_eq!(unit.sections(), ["Partition"]);
    assert_eq!(CONTRACT_VERSION, 1);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a line that is neither comment, header nor assignment is refused.
#[test]
fn a_line_that_declares_nothing_is_refused() {
    let error = UnitFile::parse("x.conf", "[Partition]\nnot an assignment\n").err();
    assert_eq!(
        error,
        Some(DefinitionError::MalformedLine {
            name: "x.conf".to_owned(),
            line: 2,
        })
    );
    assert!(error.is_some_and(|refusal| refusal.to_string().contains("x.conf:2")));
}

/// Negative: an assignment before any header has no section to belong to.
#[test]
fn an_assignment_before_any_header_is_refused() {
    assert_eq!(
        UnitFile::parse("x.conf", "Type=esp\n").err(),
        Some(DefinitionError::KeyOutsideSection {
            name: "x.conf".to_owned(),
            line: 1,
            key: "Type".to_owned(),
        })
    );
}

/// Negative: systemd merges a repeated section; this reader refuses it.
#[test]
fn a_repeated_section_is_refused() {
    let text = "[Partition]\nType=esp\n[Partition]\nLabel=ESP\n";
    assert_eq!(
        UnitFile::parse("x.conf", text).err(),
        Some(DefinitionError::DuplicateSection {
            name: "x.conf".to_owned(),
            line: 3,
            section: "Partition".to_owned(),
        })
    );
}

/// Negative: systemd takes the last assignment; this reader refuses the repeat.
#[test]
fn a_repeated_key_is_refused() {
    assert_eq!(
        UnitFile::parse("x.conf", "[Partition]\nType=esp\nType=root\n").err(),
        Some(DefinitionError::DuplicateKey {
            name: "x.conf".to_owned(),
            line: 3,
            section: "Partition".to_owned(),
            key: "Type".to_owned(),
        })
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a header must be bracketed and non-empty to open a section.
#[test]
fn only_a_well_formed_header_opens_a_section() {
    for text in ["[]\n", "[Part ition]\n", "[Partition\n", "Partition]\n"] {
        assert!(matches!(
            UnitFile::parse("x.conf", text).err(),
            Some(DefinitionError::MalformedLine { .. } | DefinitionError::KeyOutsideSection { .. })
        ));
    }
    assert!(UnitFile::parse("x.conf", "[Partition]\n").is_ok());
}

/// Boundary: a value may be empty, and whitespace on either side is trimmed.
#[test]
fn an_empty_value_is_an_assignment_and_whitespace_is_trimmed() -> Fallible {
    let unit = UnitFile::parse("x.conf", "[Partition]\n  Type = esp \nLabel=\n")?;
    assert_eq!(unit.value("Partition", "Type"), Some("esp"));
    assert_eq!(unit.value("Partition", "Label"), Some(""));
    assert_eq!(unit.value("Partition", "Missing"), None);
    assert_eq!(unit.line_of("Partition", "Missing"), None);
    Ok(())
}

/// Boundary: the longest accepted line, and the first line past the bound.
#[test]
fn the_line_length_bound_is_exact() {
    let at_limit = format!("[Partition]\n{}\n", "#".repeat(MAX_LINE_BYTES));
    assert!(UnitFile::parse("x.conf", &at_limit).is_ok());
    let past_limit = format!(
        "[Partition]\n{}\n",
        "#".repeat(MAX_LINE_BYTES.saturating_add(1))
    );
    assert!(matches!(
        UnitFile::parse("x.conf", &past_limit).err(),
        Some(DefinitionError::LineTooLong { line: 2, .. })
    ));
}

/// Repeats `line` `count` times, without appending a `format!` to a `String`.
fn repeated(count: usize, line: &dyn Fn(usize) -> String) -> String {
    (0..count).map(line).collect()
}

/// Boundary: the line-count bound refuses the first file past it.
#[test]
fn the_line_count_bound_refuses_the_first_file_past_it() {
    let many = repeated(MAX_LINES, &|index| format!("# line {index}\n"));
    assert!(UnitFile::parse("x.conf", &many).is_ok());
    let past = format!("[Partition]\n{many}");
    assert!(matches!(
        UnitFile::parse("x.conf", &past).err(),
        Some(DefinitionError::TooManyLines { bound, .. }) if bound == MAX_LINES
    ));
}

/// Boundary: the section bound refuses the first header past it.
#[test]
fn the_section_bound_refuses_the_first_header_past_it() {
    let at_limit = repeated(MAX_SECTIONS, &|index| format!("[S{index}]\n"));
    assert!(UnitFile::parse("x.conf", &at_limit).is_ok());
    let past = repeated(MAX_SECTIONS.saturating_add(1), &|index| {
        format!("[S{index}]\n")
    });
    assert!(matches!(
        UnitFile::parse("x.conf", &past).err(),
        Some(DefinitionError::TooMany {
            what: "sections",
            ..
        })
    ));
}

/// Boundary: the assignment bound refuses the first key past it.
#[test]
fn the_assignment_bound_refuses_the_first_key_past_it() {
    let keys = repeated(MAX_ENTRIES, &|index| format!("K{index}=v\n"));
    assert!(UnitFile::parse("x.conf", &format!("[Partition]\n{keys}")).is_ok());
    let past = repeated(MAX_ENTRIES.saturating_add(1), &|index| {
        format!("K{index}=v\n")
    });
    assert!(matches!(
        UnitFile::parse("x.conf", &format!("[Partition]\n{past}")).err(),
        Some(DefinitionError::TooMany {
            what: "assignments",
            ..
        })
    ));
}
