#!/usr/bin/env python3
"""Positive, negative and boundary coverage for the crate admission record.

`docs/roadmap/toolchain-admission.md` names each crate at the version it was
admitted at, and lock-file maintenance later moves the resolution inside the
declared requirement without a new admission (ledger
aegis-os-lockpin-doc-versions). The record therefore labels its versions as
history and names `Cargo.lock` as the current resolution, and every crate it
admits must still be in the lock. The cases feed text of known shape, and two
read the committed page and lock, so a reverted header or a crate that left
the lock fails here first.
"""

import pathlib
import tomllib
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
ADMISSION = ROOT / "docs" / "roadmap" / "toolchain-admission.md"
LOCK = ROOT / "Cargo.lock"
SECTION = "## Crates the workspace pins"
HEADER = "| Crate | Version at admission | How it is pinned | Role | Admitted by |"
CURRENT = "`Cargo.lock` holds the current resolution"
MAX_LINES = 4000


def crate_section(text):
    """Return the lines of the crate section, from its heading to the next one."""
    lines = text.splitlines()[:MAX_LINES]
    if SECTION not in lines:
        raise ValueError(f"no {SECTION!r} heading")
    start = lines.index(SECTION) + 1
    section = []
    for line in lines[start:]:
        if line.startswith("## "):
            break
        section.append(line)
    return section


def admitted_crates(section):
    """Return the crate names of the first table in `section`, header checked."""
    rows = []
    for line in section:
        if line.startswith("| "):
            rows.append(line)
        elif rows:
            break
    if not rows or rows[0] != HEADER:
        raise ValueError(f"crate table header is not {HEADER!r}")
    names = [row.split("|")[1].strip() for row in rows[1:] if not row.startswith("| :---")]
    if not names:
        raise ValueError("crate table holds no crate")
    return names


def missing_from_lock(names, lock_text):
    """Return the names that no [[package]] entry of the lock carries."""
    locked = {package["name"] for package in tomllib.loads(lock_text).get("package", [])}
    return [name for name in names if name not in locked]


def page(rows, prose=CURRENT):
    return "\n".join(
        (
            "# Toolchain",
            SECTION,
            "",
            f"{prose}.",
            "",
            HEADER,
            "| :--- |",
            *rows,
            "",
            "## Next",
            "| x | y |",
        )
    )


LOCK_TEXT = "\n".join(
    (
        "version = 4",
        "[[package]]",
        'name = "sha2"',
        'version = "0.11.1"',
        "[[package]]",
        'name = "syn"',
        'version = "2.0.119"',
        "[[package]]",
        'name = "syn"',
        'version = "3.0.9"',
        "",
    )
)


class AdmissionHistoryTest(unittest.TestCase):
    def test_the_committed_record_labels_versions_as_admission_history(self):
        """Positive: the committed page carries the header and names the lock."""
        section = crate_section(ADMISSION.read_text(encoding="utf-8"))
        self.assertTrue(CURRENT in "\n".join(section), f"the section must say {CURRENT}")
        self.assertTrue(admitted_crates(section))

    def test_every_committed_admission_is_still_in_the_lock(self):
        """Positive: each admitted crate is a package of the committed lock."""
        names = admitted_crates(crate_section(ADMISSION.read_text(encoding="utf-8")))
        self.assertEqual(missing_from_lock(names, LOCK.read_text(encoding="utf-8")), [])

    def test_a_crate_that_left_the_lock_is_named(self):
        """Negative: a row whose crate the lock no longer carries is reported."""
        names = admitted_crates(
            crate_section(
                page(["| sha2 | 0.11.0 | a | b | M02 |", "| md5 | 0.7.0 | a | b | M02 |"])
            )
        )
        self.assertEqual(missing_from_lock(names, LOCK_TEXT), ["md5"])

    def test_the_old_pinned_version_header_is_refused(self):
        """Negative: a table that reads its versions as current pins is refused."""
        text = page(["| sha2 | 0.11.0 | a | b | M02 |"]).replace(
            "Version at admission", "Pinned version"
        )
        with self.assertRaises(ValueError):
            admitted_crates(crate_section(text))

    def test_a_crate_locked_at_two_versions_counts_once(self):
        """Boundary: a crate the lock resolves at two versions is present."""
        names = admitted_crates(crate_section(page(["| syn | 3.0.5 | a | b | M02 |"])))
        self.assertEqual(missing_from_lock(names, LOCK_TEXT), [])

    def test_a_second_table_in_the_section_is_not_read(self):
        """Boundary: only the first table admits crates; a later licence table does not."""
        text = page(["| sha2 | 0.11.0 | a | b | M02 |"]).replace(
            "\n\n## Next", "\n\nprose\n\n| Package | Version |\n| md5 | 0.7.0 |\n\n## Next"
        )
        self.assertEqual(admitted_crates(crate_section(text)), ["sha2"])

    def test_an_empty_table_or_a_missing_heading_is_refused(self):
        """Boundary: a table without rows and a page without the section both fail."""
        with self.assertRaises(ValueError):
            admitted_crates(crate_section(page([])))
        with self.assertRaises(ValueError):
            crate_section("# Toolchain\n")


if __name__ == "__main__":
    unittest.main()
