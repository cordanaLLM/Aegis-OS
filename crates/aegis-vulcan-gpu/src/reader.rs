// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Bounded file reader helper preventing unbounded allocations on sysfs reads.

use std::io::{self, Read};
use std::path::Path;

/// Maximum size in bytes for a sysfs attribute or resource file read.
pub const MAX_ATTR_BYTES: usize = 4096;

/// Reads at most `bound` bytes of `path` as text.
///
/// A file at or past `bound` bytes is treated as malformed and returns
/// an [`io::ErrorKind::InvalidData`] error.
///
/// # Errors
///
/// Returns [`io::Error`] if the file cannot be opened, contains non-UTF-8 data,
/// or reaches or exceeds `bound` bytes.
pub fn read_bounded(path: &Path, bound: usize) -> Result<String, io::Error> {
    let file = std::fs::File::open(path)?;
    let mut text = String::new();
    let take_bound = (bound as u64).saturating_add(1);
    file.take(take_bound).read_to_string(&mut text)?;
    if text.len() >= bound {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "file at {} reaches or exceeds bound of {bound} bytes",
                path.display()
            ),
        ));
    }
    Ok(text)
}
