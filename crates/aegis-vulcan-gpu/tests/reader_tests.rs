// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Tests for bounded file reading helper.

mod common;

use std::io::ErrorKind;

use aegis_vulcan_gpu::read_bounded;
use common::{Fallible, TempSysfs};

#[test]
fn test_read_bounded_positive() -> Fallible {
    let fix = TempSysfs::new()?;
    let path = fix.root.join("small.txt");
    std::fs::write(&path, "hello world")?;

    let content = read_bounded(&path, 64)?;
    assert_eq!(content, "hello world");
    Ok(())
}

#[test]
fn test_read_bounded_negative_missing_and_oversize() -> Fallible {
    let fix = TempSysfs::new()?;
    let missing = fix.root.join("missing.txt");
    let Err(err) = read_bounded(&missing, 64) else {
        return Err("expected error on missing file".into());
    };
    assert_eq!(err.kind(), ErrorKind::NotFound);

    let oversize = fix.root.join("oversize.txt");
    std::fs::write(&oversize, "12345678901234567890")?; // 20 bytes
    let Err(err) = read_bounded(&oversize, 10) else {
        return Err("expected error on oversize file".into());
    };
    assert_eq!(err.kind(), ErrorKind::InvalidData);
    assert!(err.to_string().contains("reaches or exceeds bound of 10"));
    Ok(())
}

#[test]
fn test_read_bounded_boundary() -> Fallible {
    let fix = TempSysfs::new()?;
    let path = fix.root.join("boundary.txt");

    // Exactly 10 bytes: "0123456789"
    std::fs::write(&path, "0123456789")?;

    // Bound of 10 must fail (at or past bound)
    let Err(err) = read_bounded(&path, 10) else {
        return Err("expected error at bound".into());
    };
    assert_eq!(err.kind(), ErrorKind::InvalidData);

    // Bound of 11 must succeed (strictly below bound)
    let ok = read_bounded(&path, 11)?;
    assert_eq!(ok, "0123456789");

    // Empty file with bound 1 succeeds
    let empty = fix.root.join("empty.txt");
    std::fs::write(&empty, "")?;
    let ok_empty = read_bounded(&empty, 1)?;
    assert_eq!(ok_empty, "");

    // Empty file with bound 0 fails (0 >= 0)
    let Err(err_empty) = read_bounded(&empty, 0) else {
        return Err("expected error at bound 0".into());
    };
    assert_eq!(err_empty.kind(), ErrorKind::InvalidData);
    Ok(())
}
