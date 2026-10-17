// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Tests for DMA-BUF system heap inspection under `/dev/dma_heap/system`.

mod common;

use common::{Fallible, TempSysfs};

use aegis_vulcan_gpu::heap::inspect_dma_heap;

fn assert_absent_json(line: &str) {
    assert!(line.starts_with("RECORD heap/dma-heap-system: "));
    assert!(line.ends_with(" (recorded, never gating)"));
    assert!(line.contains("\"present\":false"));
    assert!(line.contains("\"mode\":null"));
    assert!(line.contains("\"owner_uid\":null"));
    assert!(line.contains("\"group_gid\":null"));
}

#[test]
fn heap_absent_recorded_never_gating() -> Fallible {
    let fixture = TempSysfs::new()?;
    fixture.set_dma_heap(false)?;

    let record = inspect_dma_heap(&fixture.dev);
    assert!(!record.present);
    assert_eq!(record.mode, None);
    assert_eq!(record.owner_uid, None);
    assert_eq!(record.group_gid, None);
    assert!(record.unprivileged_open.starts_with("refused:"));

    assert_absent_json(&record.format_line());
    Ok(())
}

#[test]
fn heap_present_recorded_positive() -> Fallible {
    let fixture = TempSysfs::new()?;
    fixture.set_dma_heap(true)?;

    let record = inspect_dma_heap(&fixture.dev);
    assert!(record.present);
    assert!(record.mode.is_some());
    assert!(record.owner_uid.is_some());
    assert!(record.group_gid.is_some());
    // Since it was created as a normal file in tempdir, opening it succeeds
    assert_eq!(record.unprivileged_open, "opened: ok");

    let line = record.format_line();
    assert!(line.starts_with("RECORD heap/dma-heap-system: "));
    assert!(line.ends_with(" (recorded, never gating)"));
    assert!(line.contains("\"present\":true"));
    assert!(!line.contains("\"mode\":null"));
    Ok(())
}

#[test]
fn heap_serde_boundary() -> Fallible {
    let empty = aegis_vulcan_gpu::heap::DmaHeapRecord {
        path: "/dev/dma_heap/system".to_owned(),
        present: false,
        mode: None,
        owner_uid: None,
        group_gid: None,
        unprivileged_open: "refused".to_owned(),
    };
    let json = serde_json::to_string(&empty)?;
    assert!(json.contains("\"mode\":null"));
    assert!(json.contains("\"owner_uid\":null"));
    assert!(json.contains("\"group_gid\":null"));
    let roundtrip: aegis_vulcan_gpu::heap::DmaHeapRecord = serde_json::from_str(&json)?;
    assert_eq!(empty, roundtrip);
    Ok(())
}
