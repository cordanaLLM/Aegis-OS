#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
"""Positive, negative and boundary coverage for the Concordia theme generator (HISS-15).

tools/concordia_theme.py turns ui/concordia-tokens/concordia-tokens.css into
crates/aegis-concordia/src/tokens.rs (M28, E28-5). These tests hold the
committed file to what the token file generates, so a hand-edited constant or a
token changed without regenerating fails `make verify-all`, and they hold the
reader to its allow-list: every value shape, media query and declaration it
refuses is planted here and must be refused by name. They need no Rust
toolchain and run on every leg of the platform matrix (HISS-21).
"""

import contextlib
import io
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import concordia_theme as theme

ROOT = Path(__file__).resolve().parent.parent

# The smallest token file the generator accepts: the D16 focus ring alone.
MINIMAL = ":root {\n  --concordia-focus-ring-width: 3px;\n}\n"


def with_tokens(*lines, media=""):
    """Return a token file with the focus ring plus `lines` in :root, then `media`."""
    body = "".join(f"  {line}\n" for line in ("--concordia-focus-ring-width: 3px;",) + lines)
    return f":root {{\n{body}}}\n{media}"


def run_check(source_text, committed):
    """Run `main(["--check"])` against a planted source and target; return (code, stderr)."""
    with tempfile.TemporaryDirectory() as scratch:
        source = Path(scratch) / "tokens.css"
        target = Path(scratch) / "tokens.rs"
        source.write_text(source_text, encoding="utf-8")
        if committed is not None:
            target.write_text(committed, encoding="utf-8")
        stderr = io.StringIO()
        with (
            mock.patch.object(theme, "SOURCE", source),
            mock.patch.object(theme, "TARGET", target),
            mock.patch.object(theme, "ROOT", Path(scratch)),
            contextlib.redirect_stdout(io.StringIO()),
            contextlib.redirect_stderr(stderr),
        ):
            code = theme.main(["--check"])
        return code, stderr.getvalue()


class Committed(unittest.TestCase):
    """The committed constants are exactly what the committed token file generates."""

    def test_the_committed_file_matches_the_token_file(self):
        generated = theme.render(theme.read_source(theme.SOURCE))
        self.assertEqual(theme.TARGET.read_text(encoding="utf-8"), generated)

    def test_check_passes_on_the_repository(self):
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(theme.main(["--check"]), 0)

    def test_every_token_becomes_a_constant(self):
        text = theme.COMMENT.sub("", theme.read_source(theme.SOURCE))
        declared = len(theme.TOKEN.findall(text))
        generated = theme.TARGET.read_text(encoding="utf-8")
        self.assertEqual(generated.count("pub const "), declared)

    def test_the_d16_ring_and_the_overrides_are_typed(self):
        generated = theme.TARGET.read_text(encoding="utf-8")
        self.assertIn("pub const FOCUS_RING_WIDTH: Px = Px(3);", generated)
        self.assertIn("pub const REDUCED_MOTION_MOTION_DURATION: Millis = Millis(0);", generated)
        self.assertIn("SystemColor::Highlight;", generated)


class Shapes(unittest.TestCase):
    """Each admitted value shape becomes its type."""

    def test_value_shapes(self):
        cases = {
            ("color-x", "#0a1b2c"): (
                "Rgb",
                "Rgb {\n    red: 10,\n    green: 27,\n    blue: 44,\n}",
            ),
            ("width", "12px"): ("Px", "Px(12)"),
            ("duration", "150ms"): ("Millis", "Millis(150)"),
            ("space", "0.375rem"): ("Rem", "Rem(375)"),
            ("line-height", "1.5"): ("Ratio", "Ratio(1500)"),
            ("style", "dashed"): ("LineStyle", "LineStyle::Dashed"),
            ("font-family", "system-ui, sans-serif"): (
                "&[&str]",
                '&["system-ui", "sans-serif"]',
            ),
        }
        for (name, value), expected in cases.items():
            with self.subTest(name=name):
                self.assertEqual(theme.typed(name, value), expected)

    def test_an_easing_keeps_its_sign(self):
        rust_type, rust = theme.typed("easing", "cubic-bezier(0.2, -0.5, 0, 1)")
        self.assertEqual(rust_type, "CubicBezier")
        self.assertIn("y1: -500,", rust)

    def test_a_forced_colours_override_names_a_system_colour(self):
        self.assertEqual(
            theme.typed("color-border", "CanvasText", system=True),
            ("SystemColor", "SystemColor::CanvasText"),
        )


class Refused(unittest.TestCase):
    """The allow-list refuses every shape, query and declaration it does not name."""

    def assertRefused(self, text, needle):
        with self.assertRaises(theme.TokenError) as raised:
            theme.render(text)
        self.assertIn(needle, str(raised.exception))

    def test_a_hand_edited_constant_fails_the_check(self):
        committed = theme.render(MINIMAL).replace("Px(3)", "Px(4)")
        code, stderr = run_check(MINIMAL, committed)
        self.assertEqual(code, 1)
        self.assertIn("run python3 tools/concordia_theme.py", stderr)

    def test_a_token_changed_without_regenerating_fails_the_check(self):
        committed = theme.render(MINIMAL)
        code, _ = run_check(MINIMAL.replace("3px", "4px"), committed)
        self.assertEqual(code, 1)

    def test_a_missing_target_fails_the_check(self):
        code, _ = run_check(MINIMAL, None)
        self.assertEqual(code, 1)

    def test_a_refused_source_fails_the_check(self):
        code, stderr = run_check(MINIMAL.replace("3px", "1px"), theme.render(MINIMAL))
        self.assertEqual(code, 1)
        self.assertIn("D16 floor", stderr)

    def test_unadmitted_values(self):
        cases = {
            "--concordia-color-x: rgb(1, 2, 3);": "lowercase #rrggbb",
            "--concordia-color-x: #ABCDEF;": "lowercase #rrggbb",
            "--concordia-color-x: Canvas;": "lowercase #rrggbb",
            "--concordia-width: 1.5px;": "no admitted shape",
            "--concordia-width: 3em;": "no admitted shape",
            "--concordia-width: calc(1px + 2px);": "no admitted shape",
            "--concordia-style: groove;": "no admitted shape",
            '--concordia-font-family: "Inter", sans-serif;': "unquoted names",
        }
        for line, needle in cases.items():
            with self.subTest(line=line):
                self.assertRefused(with_tokens(line), needle)

    def test_a_hex_colour_in_forced_colours_is_refused(self):
        media = "@media (forced-colors: active) {\n  :root { --concordia-color-x: #000000; }\n}\n"
        self.assertRefused(with_tokens("--concordia-color-x: #ffffff;", media=media), "system")

    def test_an_unknown_media_query_is_refused(self):
        media = "@media (prefers-color-scheme: dark) {\n  :root { --concordia-width: 1px; }\n}\n"
        self.assertRefused(with_tokens(media=media), "not an admitted override")

    def test_a_media_block_without_root_is_refused(self):
        media = "@media (prefers-reduced-motion: reduce) {\n  * { animation: none; }\n}\n"
        self.assertRefused(with_tokens(media=media), "has no :root override")

    def test_an_override_of_an_unknown_token_is_refused(self):
        media = "@media (prefers-reduced-motion: reduce) {\n  :root { --concordia-gone: 0ms; }\n}\n"
        self.assertRefused(with_tokens(media=media), "overrides no :root token")

    def test_a_duplicate_token_is_refused(self):
        self.assertRefused(
            with_tokens("--concordia-width: 1px;", "--concordia-width: 2px;"), "twice"
        )

    def test_a_file_without_root_or_ring_is_refused(self):
        self.assertRefused("body { color: red; }\n", "no :root block")
        self.assertRefused(":root {\n  --concordia-width: 1px;\n}\n", "is missing")
        self.assertRefused(":root {\n  --concordia-focus-ring-width: 3px;\n", "not closed")

    def test_a_ring_that_is_not_pixels_is_refused(self):
        self.assertRefused(MINIMAL.replace("3px", "0.25rem"), "D16 floor")


class LineEnds(unittest.TestCase):
    """A CRLF checkout generates the same file as an LF one (HISS-21)."""

    def test_a_crlf_source_generates_the_lf_result(self):
        with tempfile.TemporaryDirectory() as scratch:
            path = Path(scratch) / "tokens.css"
            path.write_bytes(MINIMAL.replace("\n", "\r\n").encode("utf-8"))
            self.assertEqual(theme.render(theme.read_source(path)), theme.render(MINIMAL))

    def test_the_check_names_the_target_with_forward_slashes(self):
        code, stderr = run_check(MINIMAL, "")
        self.assertEqual(code, 1)
        self.assertNotIn("\\", stderr)


class Boundary(unittest.TestCase):
    """The D16 floor and the size and count bounds sit exactly where they say."""

    def test_the_floor_admits_two_pixels_and_refuses_one(self):
        self.assertIn("Px(2)", theme.render(MINIMAL.replace("3px", "2px")))
        with self.assertRaises(theme.TokenError):
            theme.render(MINIMAL.replace("3px", "1px"))

    def test_the_token_count_bound(self):
        lines = [f"--concordia-w{index}: 1px;" for index in range(theme.MAX_TOKENS - 1)]
        self.assertIn("W0", theme.render(with_tokens(*lines)))
        with self.assertRaises(theme.TokenError):
            theme.render(with_tokens(*lines, "--concordia-one-more: 1px;"))

    def test_the_source_size_bound(self):
        with tempfile.TemporaryDirectory() as scratch:
            path = Path(scratch) / "tokens.css"
            path.write_bytes(b" " * theme.MAX_SOURCE_BYTES)
            self.assertEqual(len(theme.read_source(path)), theme.MAX_SOURCE_BYTES)
            path.write_bytes(b" " * (theme.MAX_SOURCE_BYTES + 1))
            with self.assertRaises(theme.TokenError):
                theme.read_source(path)

    def test_four_digit_values_and_three_decimals(self):
        self.assertEqual(theme.typed("width", "9999px"), ("Px", "Px(9999)"))
        self.assertEqual(theme.typed("space", "0.001rem"), ("Rem", "Rem(1)"))
        for value in ("10000px", "0.0001rem", "01px"):
            with self.subTest(value=value), self.assertRaises(theme.TokenError):
                theme.typed("width", value)


if __name__ == "__main__":
    unittest.main()
