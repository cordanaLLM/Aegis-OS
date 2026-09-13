// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The systemd drop-in reader both definition kinds are built on.
//!
//! It reads the subset of the unit-file syntax that `repart.d(5)` and
//! `sysupdate.d(5)` drop-ins use: comment lines, `[Section]` headers and
//! `Key=Value` assignments. Line continuations, quoting and `Key=` resets are
//! deliberately absent; none of the reviewed definitions uses them, and a
//! reader that silently accepted syntax it does not implement would report a
//! definition as understood when it is not.
//!
//! Every loop below is bounded by a constant declared in this module.

use crate::error::DefinitionError;

/// Scalar bound on the lines one definition file may hold.
pub const MAX_LINES: usize = 512;

/// Scalar bound on the bytes one line may hold.
pub const MAX_LINE_BYTES: usize = 512;

/// Scalar bound on the sections one definition file may declare.
pub const MAX_SECTIONS: usize = 8;

/// Scalar bound on the assignments one definition file may hold.
pub const MAX_ENTRIES: usize = 128;

/// One `Key=Value` assignment, with the section and line it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The section header this assignment sits under, without brackets.
    pub section: String,
    /// The key, as written.
    pub key: String,
    /// The value, trimmed of surrounding whitespace.
    pub value: String,
    /// The 1-based line number the assignment was read from.
    pub line: usize,
}

/// A parsed drop-in: its section order and its assignments, in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitFile {
    name: String,
    sections: Vec<String>,
    entries: Vec<Entry>,
}

/// Splits a `Key=Value` line, returning the trimmed key and value.
fn split_assignment(text: &str) -> Option<(&str, &str)> {
    let (key, value) = text.split_once('=')?;
    let key = key.trim();
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    Some((key, value.trim()))
}

/// Returns the section name a header line declares, without its brackets.
fn section_header(text: &str) -> Option<&str> {
    let inner = text.strip_prefix('[')?.strip_suffix(']')?;
    if inner.is_empty() || !inner.chars().all(char::is_alphanumeric) {
        return None;
    }
    Some(inner)
}

/// Returns `true` when the line carries no declaration.
fn is_blank_or_comment(text: &str) -> bool {
    text.is_empty() || text.starts_with('#') || text.starts_with(';')
}

impl UnitFile {
    /// Parses `text` as the drop-in named `name`.
    ///
    /// # Errors
    ///
    /// Returns [`DefinitionError`] when a bound is passed, a line is neither a
    /// comment nor a header nor an assignment, an assignment precedes every
    /// header, or a section or key repeats.
    pub fn parse(name: &str, text: &str) -> Result<Self, DefinitionError> {
        let mut unit = Self {
            name: name.to_owned(),
            sections: Vec::new(),
            entries: Vec::new(),
        };
        unit.count_lines(text)?;
        let mut current: Option<String> = None;
        for (index, raw) in text.lines().take(MAX_LINES).enumerate() {
            let line = index.saturating_add(1);
            unit.check_line_length(line, raw)?;
            let trimmed = raw.trim();
            if is_blank_or_comment(trimmed) {
                continue;
            }
            if let Some(header) = section_header(trimmed) {
                unit.open_section(line, header)?;
                current = Some(header.to_owned());
                continue;
            }
            unit.add_entry(line, trimmed, current.as_deref())?;
        }
        Ok(unit)
    }

    /// Refuses a file with more lines than [`MAX_LINES`].
    fn count_lines(&self, text: &str) -> Result<(), DefinitionError> {
        let lines = text.lines().take(MAX_LINES.saturating_add(1)).count();
        if lines > MAX_LINES {
            return Err(DefinitionError::TooManyLines {
                name: self.name.clone(),
                lines,
                bound: MAX_LINES,
            });
        }
        Ok(())
    }

    /// Refuses a line longer than [`MAX_LINE_BYTES`].
    fn check_line_length(&self, line: usize, raw: &str) -> Result<(), DefinitionError> {
        if raw.len() > MAX_LINE_BYTES {
            return Err(DefinitionError::LineTooLong {
                name: self.name.clone(),
                line,
                bound: MAX_LINE_BYTES,
            });
        }
        Ok(())
    }

    /// Records a section header, refusing a repeat or an overflow.
    fn open_section(&mut self, line: usize, header: &str) -> Result<(), DefinitionError> {
        if self.sections.iter().any(|known| known == header) {
            return Err(DefinitionError::DuplicateSection {
                name: self.name.clone(),
                line,
                section: header.to_owned(),
            });
        }
        if self.sections.len() >= MAX_SECTIONS {
            return Err(DefinitionError::TooMany {
                name: self.name.clone(),
                what: "sections",
                bound: MAX_SECTIONS,
            });
        }
        self.sections.push(header.to_owned());
        Ok(())
    }

    /// Records one assignment, refusing a malformed line, a stray key or a repeat.
    fn add_entry(
        &mut self,
        line: usize,
        trimmed: &str,
        section: Option<&str>,
    ) -> Result<(), DefinitionError> {
        let Some((key, value)) = split_assignment(trimmed) else {
            return Err(DefinitionError::MalformedLine {
                name: self.name.clone(),
                line,
            });
        };
        let Some(section) = section else {
            return Err(DefinitionError::KeyOutsideSection {
                name: self.name.clone(),
                line,
                key: key.to_owned(),
            });
        };
        if self.value(section, key).is_some() {
            return Err(DefinitionError::DuplicateKey {
                name: self.name.clone(),
                line,
                section: section.to_owned(),
                key: key.to_owned(),
            });
        }
        if self.entries.len() >= MAX_ENTRIES {
            return Err(DefinitionError::TooMany {
                name: self.name.clone(),
                what: "assignments",
                bound: MAX_ENTRIES,
            });
        }
        self.entries.push(Entry {
            section: section.to_owned(),
            key: key.to_owned(),
            value: value.to_owned(),
            line,
        });
        Ok(())
    }

    /// The file name this unit was parsed under.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The section headers, in the order the file declares them.
    #[must_use]
    pub fn sections(&self) -> &[String] {
        &self.sections
    }

    /// Every assignment, in file order.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The value assigned to `key` in `section`, if any.
    #[must_use]
    pub fn value(&self, section: &str, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|entry| entry.section == section && entry.key == key)
            .map(|entry| entry.value.as_str())
    }

    /// The line `key` was assigned on in `section`, if any.
    #[must_use]
    pub fn line_of(&self, section: &str, key: &str) -> Option<usize> {
        self.entries
            .iter()
            .find(|entry| entry.section == section && entry.key == key)
            .map(|entry| entry.line)
    }

    /// Renders the assignments back as a canonical drop-in.
    ///
    /// Comments are not retained: the canonical form is what the file
    /// declares, which is what a round-trip has to preserve.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        for section in self.sections.iter().take(MAX_SECTIONS) {
            self.render_section(&mut out, section);
        }
        out
    }

    /// Appends one section and the assignments that belong to it.
    fn render_section(&self, out: &mut String, section: &str) {
        out.push('[');
        out.push_str(section);
        out.push_str("]\n");
        let owned = self
            .entries
            .iter()
            .take(MAX_ENTRIES)
            .filter(|entry| entry.section == section);
        for entry in owned {
            out.push_str(&entry.key);
            out.push('=');
            out.push_str(&entry.value);
            out.push('\n');
        }
        out.push('\n');
    }
}
