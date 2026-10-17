# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2

"""Generate the Concordia theme constants the native shell compiles (M28, E28-5).

ui/concordia-tokens/concordia-tokens.css is the one token source M04 committed
(D05, D101). This script reads its ``:root`` block and the two media blocks
that override it, and writes crates/aegis-concordia/src/tokens.rs: one Rust
constant per token, typed by the value's grammar.

The reader is an allow-list: a value is accepted only in one of the shapes
below, a media block only when its query is one of ``MEDIA``, and anything else
is refused with the token's name. The D16 focus ring must be at least
``FOCUS_RING_FLOOR_PX`` wide; a narrower one is refused rather than emitted.

``--check`` regenerates in memory and exits 1 when the committed file differs,
so a hand-edited constant and a token changed without regenerating both fail
``make verify-all`` (tools/test_concordia_theme.py runs it there).
"""

import argparse
import hashlib
import pathlib
import re
import sys

import host

ROOT = pathlib.Path(__file__).resolve().parent.parent
SOURCE = ROOT / "ui" / "concordia-tokens" / "concordia-tokens.css"
TARGET = ROOT / "crates" / "aegis-concordia" / "src" / "tokens.rs"
SOURCE_NAME = "ui/concordia-tokens/concordia-tokens.css"

# D16: the focus ring is 3 px; 2 px is the floor the boundary test holds it to.
FOCUS_RING_TOKEN = "focus-ring-width"
FOCUS_RING_FLOOR_PX = 2

# The token file is 94 lines; a source a hundred times that is not this file.
MAX_SOURCE_BYTES = 1 << 16
MAX_TOKENS = 256

# Media queries whose :root overrides become a second constant set, and the
# prefix their constants take. Any other query is refused.
MEDIA = {
    "prefers-reduced-motion: reduce": "REDUCED_MOTION_",
    "forced-colors: active": "FORCED_COLORS_",
}

# CSS system colours (CSS Color 4) a forced-colours override may name.
SYSTEM_COLORS = {
    "Canvas": "Canvas",
    "CanvasText": "CanvasText",
    "Highlight": "Highlight",
    "HighlightText": "HighlightText",
    "ButtonFace": "ButtonFace",
    "ButtonText": "ButtonText",
    "LinkText": "LinkText",
    "GrayText": "GrayText",
}
LINE_STYLES = {"solid": "Solid", "dashed": "Dashed", "dotted": "Dotted", "double": "Double"}

TOKEN = re.compile(r"--concordia-([a-z0-9]+(?:-[a-z0-9]+)*)\s*:\s*([^;]+);")
COMMENT = re.compile(r"/\*.*?\*/", re.S)
HEX = re.compile(r"#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})")
INTEGER = r"(0|[1-9][0-9]{0,3})"
NUMBER = INTEGER + r"(?:\.([0-9]{1,3}))?"
PX = re.compile(INTEGER + "px")
MS = re.compile(INTEGER + "ms")
REM = re.compile(NUMBER + "rem")
RATIO = re.compile(NUMBER)
SIGNED = r"\s*(-?)" + NUMBER + r"\s*"
BEZIER = re.compile(r"cubic-bezier\(" + ",".join([SIGNED] * 4) + r"\)")
FAMILY = re.compile(r"[a-z][a-z0-9-]*")


class TokenError(ValueError):
    """A token file the generator refuses, naming the token or block."""


def milli(whole, fraction):
    """Return a decimal written as whole.fraction in thousandths."""
    return int(whole) * 1000 + int((fraction or "").ljust(3, "0"))


def struct(name, **fields):
    """Render a struct literal the way rustfmt lays out a const initialiser."""
    body = "".join(f"    {field}: {value},\n" for field, value in fields.items())
    return f"{name} {{\n{body}}}"


def bezier(match):
    """Return the four control values of a cubic-bezier() in thousandths."""
    groups = match.groups()
    values = []
    for index in range(4):
        sign, whole, fraction = groups[index * 3 : index * 3 + 3]
        value = milli(whole, fraction)
        values.append(-value if sign else value)
    return values


def color(name, value, system):
    """Type a colour: a six-digit hex in the base set, a system colour in an override."""
    match = HEX.fullmatch(value)
    if match and not system:
        red, green, blue = (int(part, 16) for part in match.groups())
        return "Rgb", struct("Rgb", red=red, green=green, blue=blue)
    if value in SYSTEM_COLORS and system:
        return "SystemColor", f"SystemColor::{SYSTEM_COLORS[value]}"
    shape = "a CSS system colour" if system else "a lowercase #rrggbb colour"
    raise TokenError(f"--concordia-{name}: {value!r} is not {shape}")


def font_family(name, value):
    """Type a font-family list of unquoted generic or family names."""
    families = [part.strip() for part in value.split(",")]
    if not families or not all(FAMILY.fullmatch(part) for part in families):
        raise TokenError(f"--concordia-{name}: {value!r} is not a list of unquoted names")
    listed = ", ".join(f'"{part}"' for part in families)
    return "&[&str]", f"&[{listed}]"


def measure(name, value):
    """Type a length, duration, ratio, line style or easing; refuse anything else."""
    if match := PX.fullmatch(value):
        return "Px", f"Px({int(match[1])})"
    if match := MS.fullmatch(value):
        return "Millis", f"Millis({int(match[1])})"
    if match := REM.fullmatch(value):
        return "Rem", f"Rem({milli(match[1], match[2])})"
    if match := RATIO.fullmatch(value):
        return "Ratio", f"Ratio({milli(match[1], match[2])})"
    if value in LINE_STYLES:
        return "LineStyle", f"LineStyle::{LINE_STYLES[value]}"
    if match := BEZIER.fullmatch(value):
        x1, y1, x2, y2 = bezier(match)
        return "CubicBezier", struct("CubicBezier", x1=x1, y1=y1, x2=x2, y2=y2)
    raise TokenError(f"--concordia-{name}: {value!r} has no admitted shape")


def typed(name, value, system=False):
    """Return (Rust type, Rust expression) for one token value."""
    value = " ".join(value.split())
    if name.startswith("color-"):
        return color(name, value, system)
    if name == "font-family":
        return font_family(name, value)
    return measure(name, value)


def floor_check(name, rust):
    """Refuse a focus ring narrower than the D16 floor."""
    if name != FOCUS_RING_TOKEN:
        return
    width = int(rust[3:-1]) if rust.startswith("Px(") else -1
    if width < FOCUS_RING_FLOOR_PX:
        raise TokenError(
            f"--concordia-{name}: the focus ring is narrower than the "
            f"{FOCUS_RING_FLOOR_PX}px D16 floor"
        )


def tokens_of(body, label, system=False):
    """Return [(name, type, expression)] for the custom properties in one block."""
    found = []
    seen = set()
    for match in TOKEN.finditer(body):
        name = match[1]
        if name in seen:
            raise TokenError(f"{label}: --concordia-{name} is declared twice")
        seen.add(name)
        rust_type, rust = typed(name, match[2], system)
        floor_check(name, rust)
        found.append((name, rust_type, rust))
    if len(found) > MAX_TOKENS:
        raise TokenError(f"{label}: more than {MAX_TOKENS} tokens")
    return found


def root_body(text, start):
    """Return (body, end) of the `:root { ... }` block opening at or after `start`."""
    opening = re.compile(r":root\s*\{").search(text, start)
    if opening is None:
        return None, start
    end = text.find("}", opening.end())
    if end < 0:
        raise TokenError(":root block is not closed")
    return text[opening.end() : end], end


def media_sets(text):
    """Return [(prefix, tokens)] for each admitted media block's :root override."""
    sets = []
    for match in re.finditer(r"@media\s*\(([^)]*)\)\s*\{", text):
        query = " ".join(match[1].split())
        if query not in MEDIA:
            raise TokenError(f"@media ({query}) is not an admitted override")
        body, _ = root_body(text, match.end())
        if body is None:
            raise TokenError(f"@media ({query}) has no :root override")
        system = query == "forced-colors: active"
        sets.append((MEDIA[query], tokens_of(body, f"@media ({query})", system)))
    return sets


def parse(text):
    """Return (base tokens, [(prefix, override tokens)]) from the token file's text."""
    text = COMMENT.sub("", text)
    base_body, base_end = root_body(text, 0)
    if base_body is None:
        raise TokenError("no :root block")
    base = tokens_of(base_body, ":root")
    if FOCUS_RING_TOKEN not in {name for name, _, _ in base}:
        raise TokenError(f"--concordia-{FOCUS_RING_TOKEN} is missing from :root")
    known = {name: rust_type for name, rust_type, _ in base}
    overrides = media_sets(text[base_end + 1 :])
    for prefix, tokens in overrides:
        for name, _, _ in tokens:
            if name not in known:
                raise TokenError(f"{prefix}: --concordia-{name} overrides no :root token")
    return base, overrides


HEADER = """\
// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The Concordia tokens as Rust constants.
//!
//! @generated by `tools/concordia_theme.py` from `{source}`,
//! sha256 {digest}.
//! Do not edit: change the token file and run `python3 tools/concordia_theme.py`;
//! `make verify-all` fails on a difference.

use crate::{{{imports}}};
"""
TYPES = ("CubicBezier", "LineStyle", "Millis", "Px", "Ratio", "Rem", "Rgb", "SystemColor")


def constant(prefix, name, rust_type, rust):
    """Render one constant with its source token, and media query, as its doc line."""
    ident = prefix + name.upper().replace("-", "_")
    query = {value: key for key, value in MEDIA.items()}.get(prefix)
    where = f" under `@media ({query})`" if query else ""
    return f"\n/// `--concordia-{name}`{where}.\npub const {ident}: {rust_type} = {rust};\n"


def render(text):
    """Return the generated Rust source for the token file's text."""
    base, overrides = parse(text)
    digest = hashlib.sha256(text.encode("utf-8")).hexdigest()
    body = "".join(constant("", *token) for token in base)
    for prefix, tokens in overrides:
        body += "".join(constant(prefix, *token) for token in tokens)
    imports = ", ".join(name for name in TYPES if f": {name} =" in body)
    return HEADER.format(source=SOURCE_NAME, digest=digest, imports=imports) + body


def read_source(path):
    """Read the token file, bounded, with LF line ends whatever the checkout wrote.

    A Windows checkout may deliver the file with CRLF; the digest in the header
    and the generated file must not depend on that (HISS-21).
    """
    data = path.read_bytes()
    if len(data) > MAX_SOURCE_BYTES:
        raise TokenError(f"{host.target(path)} is larger than {MAX_SOURCE_BYTES} bytes")
    return data.decode("utf-8").replace("\r\n", "\n")


def main(argv=None):
    """Write, or with --check compare, the generated constants."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true", help="compare, do not write")
    args = parser.parse_args(argv)
    try:
        generated = render(read_source(SOURCE))
    except TokenError as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    if not args.check:
        TARGET.write_text(generated, encoding="utf-8", newline="\n")
        print(f"wrote {host.target(TARGET.relative_to(ROOT))}")
        return 0
    committed = TARGET.read_text(encoding="utf-8") if TARGET.is_file() else ""
    target = host.target(TARGET.relative_to(ROOT))
    if committed != generated:
        print(
            f"FAIL: {target} differs from what {SOURCE_NAME} generates; "
            "run python3 tools/concordia_theme.py",
            file=sys.stderr,
        )
        return 1
    print(f"PASS: {target} matches {SOURCE_NAME}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
