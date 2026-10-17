// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The hardened `justitia-interceptor` unit, reviewed as a contract.
//!
//! The unit is a contract file, not an installed unit: it is read here, never
//! copied to a systemd path, never enabled and never started. What these tests
//! exercise is the review, so "the unit declares `IPAddressDeny=any`" fails
//! when somebody deletes the line rather than when somebody re-reads the file.

use std::path::{Path, PathBuf};

use aegis_justitia::{
    CONTRACT_MARKER, INSTALL_DIRECTORIES, MAX_UNIT_BYTES, MAX_UNIT_LINES, REQUIRED_DIRECTIVES,
    START_LIMIT_DIRECTIVES, UnitContractError, is_installed_path, review,
};

/// The contract file's path inside the crate.
const UNIT_RELATIVE_PATH: &str = "contracts/justitia-interceptor.service";

/// The directive the P06 report ties to the Zero-SaaS requirement.
const ZERO_SAAS_DIRECTIVE: &str = "IPAddressDeny=any";

/// Returns the tracked contract file's path.
fn unit_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(UNIT_RELATIVE_PATH)
}

/// Reads the tracked contract file.
fn unit_text() -> String {
    std::fs::read_to_string(unit_path()).unwrap_or_default()
}

/// The start-limit lines the tracked unit declares (issue #232).
const INTERVAL_LINE: &str = "StartLimitIntervalSec=10s";
const BURST_LINE: &str = "StartLimitBurst=5";

/// The smallest unit text the review accepts, with the two start-limit lines
/// given in its `[Unit]` section.
fn minimal_unit(interval: &str, burst: &str) -> String {
    let mut lines: Vec<String> = vec![
        format!("# {CONTRACT_MARKER}"),
        "[Unit]".to_owned(),
        interval.to_owned(),
        burst.to_owned(),
        "[Service]".to_owned(),
    ];
    for directive in REQUIRED_DIRECTIVES {
        lines.push((*directive).to_owned());
    }
    lines.join("\n")
}

/// Replaces the one tracked line `from` with `to` and checks it was there.
fn replace_line(text: &str, from: &str, to: &str) -> String {
    assert_eq!(
        text.matches(from).count(),
        1,
        "the fixture must hold {from} once"
    );
    text.replace(from, to)
}

/// The refusal for one start-limit setting, by kind.
fn invalid(directive: &'static str) -> Result<(), UnitContractError> {
    Err(UnitContractError::InvalidStartLimit { directive })
}

// --- Positive -------------------------------------------------------------

/// Positive: the tracked unit passes its own review and declares the directive
/// the requirement names.
#[test]
fn the_tracked_unit_declares_the_hardening_the_contract_requires() {
    let text = unit_text();
    assert!(!text.is_empty(), "the contract file must be tracked");
    assert_eq!(review(&text), Ok(()));
    assert!(text.contains(ZERO_SAAS_DIRECTIVE));
    assert!(text.contains(CONTRACT_MARKER));
    for directive in REQUIRED_DIRECTIVES {
        assert!(
            text.contains(directive),
            "the unit must declare {directive}"
        );
    }
}

/// Positive: the header says what the file is, so a reader who finds it cannot
/// mistake it for an installed unit.
#[test]
fn the_header_says_the_file_is_a_contract_and_not_an_installed_unit() {
    let text = unit_text();
    let header: String = text.lines().take(24).collect::<Vec<&str>>().join("\n");
    assert!(header.contains(CONTRACT_MARKER));
    assert!(header.contains("not an installed unit"));
    assert!(
        header.contains("export-015"),
        "the header cites the source of the directive"
    );
}

// --- Negative -------------------------------------------------------------

/// Negative: removing the directive fails the review, and the refusal names it.
#[test]
fn removing_the_zero_saas_directive_fails_the_review() {
    let text = unit_text();
    let stripped: String = text
        .lines()
        .filter(|line| line.trim() != ZERO_SAAS_DIRECTIVE)
        .collect::<Vec<&str>>()
        .join("\n");
    assert_ne!(text, stripped, "the fixture must remove something");
    assert_eq!(
        review(&stripped),
        Err(UnitContractError::MissingDirective {
            directive: ZERO_SAAS_DIRECTIVE
        })
    );
}

/// Negative: removing the contract marker fails the review too, so a copy that
/// has lost the warning cannot pass as the contract.
#[test]
fn removing_the_contract_marker_fails_the_review() {
    let text = unit_text();
    let stripped = text.replace(CONTRACT_MARKER, "");
    assert_eq!(
        review(&stripped),
        Err(UnitContractError::MissingContractMarker)
    );
}

/// Negative: a directive that survives only inside a comment does not count as
/// declared, so commenting the hardening out is not a way past the review.
#[test]
fn a_commented_out_directive_does_not_count_as_declared() {
    let text = unit_text();
    let commented = text.replace(
        &format!("\n{ZERO_SAAS_DIRECTIVE}"),
        &format!("\n# {ZERO_SAAS_DIRECTIVE}"),
    );
    assert!(commented.contains(ZERO_SAAS_DIRECTIVE));
    assert_eq!(
        review(&commented),
        Err(UnitContractError::MissingDirective {
            directive: ZERO_SAAS_DIRECTIVE
        })
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the reviewer reads exactly up to its byte bound and refuses one
/// byte past it.
#[test]
fn the_reviewer_reads_up_to_its_byte_bound_and_no_further() {
    let filler = "#\n".repeat(MAX_UNIT_BYTES);
    let at_bound = filler.get(..MAX_UNIT_BYTES).map(str::to_owned);
    let at_bound = at_bound.unwrap_or_default();
    assert_eq!(at_bound.len(), MAX_UNIT_BYTES);
    assert_eq!(
        review(&at_bound),
        Err(UnitContractError::MissingContractMarker)
    );

    let over_bound = format!("{at_bound}#");
    assert_eq!(
        review(&over_bound),
        Err(UnitContractError::TooLong {
            max: MAX_UNIT_BYTES,
            actual: MAX_UNIT_BYTES.saturating_add(1)
        })
    );
}

/// Boundary: the reviewer reads exactly the declared number of lines. A unit
/// whose hardening sits past that line bound fails, which is the fail-closed
/// reading of a bounded scan.
#[test]
fn the_reviewer_reads_up_to_its_line_bound_and_no_further() {
    let inside = minimal_unit("StartLimitIntervalSec=10s", "StartLimitBurst=5");
    assert!(inside.lines().count() < MAX_UNIT_LINES);
    assert_eq!(review(&inside), Ok(()));

    let padding = "\n".repeat(MAX_UNIT_LINES);
    let outside = format!("# {CONTRACT_MARKER}{padding}{ZERO_SAAS_DIRECTIVE}");
    assert!(outside.lines().count() > MAX_UNIT_LINES);
    assert!(review(&outside).is_err());
}

/// Boundary: the contract file is not installed. It sits inside the crate, not
/// in any directory systemd loads units from, and nothing starts it.
#[test]
fn the_contract_file_is_not_at_an_installed_unit_path() {
    let path = unit_path().display().to_string();
    assert!(path.ends_with(UNIT_RELATIVE_PATH));
    assert!(
        !is_installed_path(&path),
        "the contract file must not live where systemd would load it"
    );
    for directory in INSTALL_DIRECTORIES {
        let installed = format!("{directory}justitia-interceptor.service");
        assert!(is_installed_path(&installed));
    }
    assert!(!is_installed_path(
        "/home/someone/justitia-interceptor.service"
    ));
}

/// Boundary: the binary the unit would start is not built by this milestone, so
/// there is nothing here that could be executed even by accident.
#[test]
fn the_execstart_binary_is_not_built_by_this_milestone() {
    let text = unit_text();
    assert!(text.contains("ExecStart=/usr/bin/justitia-interceptor"));
    let manifest =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
            .unwrap_or_default();
    assert!(
        !manifest.contains("[[bin]]"),
        "the crate ships a library; the interceptor daemon is later work"
    );
    assert!(
        !Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/main.rs")
            .exists()
    );
}

// --- Start rate limit (issue #232) -----------------------------------------

/// Positive: the tracked unit writes its start limit down in `[Unit]`, beside
/// `Restart=always`, instead of inheriting the manager's default.
#[test]
fn the_tracked_unit_writes_its_start_limit_down() {
    let text = unit_text();
    assert_eq!(review(&text), Ok(()));
    assert_eq!(
        START_LIMIT_DIRECTIVES,
        ["StartLimitIntervalSec", "StartLimitBurst"]
    );
    let unit_section = text.split("[Service]").next().unwrap_or_default();
    assert!(unit_section.contains(&format!("\n{INTERVAL_LINE}\n")));
    assert!(unit_section.contains(&format!("\n{BURST_LINE}\n")));
    assert!(text.contains("\nRestart=always\n"));
}

/// Negative: each setting is required, and a commented-out one is absent.
#[test]
fn a_missing_start_limit_setting_fails_the_review() {
    let text = unit_text();
    for (line, directive) in [
        (INTERVAL_LINE, "StartLimitIntervalSec"),
        (BURST_LINE, "StartLimitBurst"),
    ] {
        let commented = replace_line(&text, &format!("\n{line}\n"), &format!("\n# {line}\n"));
        assert_eq!(
            review(&commented),
            Err(UnitContractError::MissingStartLimit { directive })
        );
    }
}

/// Negative: switching the limit off, or naming no bound, fails the review.
#[test]
fn a_disabled_or_unbounded_start_limit_fails_the_review() {
    let text = unit_text();
    for value in ["0", "0s", "infinity"] {
        let planted = replace_line(
            &text,
            INTERVAL_LINE,
            &format!("StartLimitIntervalSec={value}"),
        );
        assert_eq!(
            review(&planted),
            invalid("StartLimitIntervalSec"),
            "{value}"
        );
    }
    let planted = replace_line(&text, BURST_LINE, "StartLimitBurst = 0");
    assert_eq!(review(&planted), invalid("StartLimitBurst"));
}

/// Negative: a second declaration, the older `StartLimitInterval=` spelling
/// included, could override the value reviewed, so it fails the review.
#[test]
fn a_repeated_start_limit_setting_fails_the_review() {
    let text = unit_text();
    let legacy = replace_line(
        &text,
        INTERVAL_LINE,
        &format!("{INTERVAL_LINE}\nStartLimitInterval=0"),
    );
    assert_eq!(
        review(&legacy),
        Err(UnitContractError::RepeatedStartLimit {
            directive: "StartLimitIntervalSec"
        })
    );
    let twice = replace_line(&text, BURST_LINE, &format!("{BURST_LINE}\n{BURST_LINE}"));
    assert_eq!(
        review(&twice),
        Err(UnitContractError::RepeatedStartLimit {
            directive: "StartLimitBurst"
        })
    );
}

/// Negative: a start-limit setting outside `[Unit]` fails the review.
#[test]
fn a_start_limit_setting_outside_the_unit_section_fails_the_review() {
    let text = unit_text();
    let removed = replace_line(&text, &format!("\n{BURST_LINE}\n"), "\n");
    let moved = replace_line(
        &removed,
        "\nRestart=always\n",
        &format!("\nRestart=always\n{BURST_LINE}\n"),
    );
    assert_eq!(
        review(&moved),
        Err(UnitContractError::MisplacedStartLimit {
            directive: "StartLimitBurst"
        })
    );
}

/// Boundary: the smallest positive values pass; zero, signs, other time units
/// and empty values are refused.
#[test]
fn the_start_limit_values_stop_at_one() {
    assert_eq!(
        review(&minimal_unit(
            "StartLimitIntervalSec=1",
            "StartLimitBurst=1"
        )),
        Ok(())
    );
    assert_eq!(
        review(&minimal_unit("StartLimitIntervalSec=1s", BURST_LINE)),
        Ok(())
    );
    for value in ["00", "-1", "1min", "", "s", "1ss"] {
        let interval = format!("StartLimitIntervalSec={value}");
        assert_eq!(
            review(&minimal_unit(&interval, BURST_LINE)),
            invalid("StartLimitIntervalSec"),
            "{value}"
        );
    }
    for value in ["0", "1s", "+1", ""] {
        let burst = format!("StartLimitBurst={value}");
        assert_eq!(
            review(&minimal_unit(INTERVAL_LINE, &burst)),
            invalid("StartLimitBurst"),
            "{value}"
        );
    }
}
