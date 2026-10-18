// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Inspection of DMA-BUF system heap under `/dev/dma_heap/system`.

use std::os::unix::fs::MetadataExt;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Record of `/dev/dma_heap/system` status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DmaHeapRecord {
    /// Canonical or observed path to the `dma_heap` device node.
    pub path: String,
    /// Whether the device node is present on the filesystem.
    pub present: bool,
    /// File mode in octal string format (e.g. "0o600"), or `None` if absent.
    pub mode: Option<String>,
    /// Owner UID, or `None` if absent.
    pub owner_uid: Option<u32>,
    /// Group GID, or `None` if absent.
    pub group_gid: Option<u32>,
    /// Result of an unprivileged read-only open attempt.
    pub unprivileged_open: String,
}

impl DmaHeapRecord {
    /// Formats the single-line summary for gate logging.
    #[must_use]
    pub fn format_line(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_else(|_| "{}".to_owned());
        format!("RECORD heap/dma-heap-system: {json} (recorded, never gating)")
    }
}

/// Inspects the DMA heap node at `<dev_root>/dma_heap/system`.
///
/// This check is purely observational and never gates (errors are recorded as strings).
#[must_use]
pub fn inspect_dma_heap(dev_root: &Path) -> DmaHeapRecord {
    let heap_path = dev_root.join("dma_heap").join("system");
    let path_str = heap_path.to_string_lossy().into_owned();

    let (present, mode, owner_uid, group_gid) = match std::fs::metadata(&heap_path) {
        Ok(meta) => {
            let octal = format!("0o{:o}", meta.mode() & 0o777);
            (true, Some(octal), Some(meta.uid()), Some(meta.gid()))
        }
        Err(_) => (false, None, None, None),
    };

    let unprivileged_open = match std::fs::File::open(&heap_path) {
        Ok(_) => "opened: ok".to_owned(),
        Err(err) => match err.raw_os_error() {
            Some(code) => {
                let err_str = err.to_string();
                let clean_msg = err_str.split(" (os error").next().unwrap_or(&err_str);
                format!("refused: errno {code} ({clean_msg})")
            }
            None => format!("refused: {err}"),
        },
    };

    DmaHeapRecord {
        path: path_str,
        present,
        mode,
        owner_uid,
        group_gid,
        unprivileged_open,
    }
}
