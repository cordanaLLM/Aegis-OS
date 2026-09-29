#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
"""The published JSON Schemas (D105), read the way a consumer that is not Rust reads them.

crates/aegis-fabrica-defs/tests/json_schema.rs holds each committed schema to
what the contract types generate. This file reads them from the other side:
the payloads Aegis publishes validate against them, the array form, an unknown
field and a value outside a bound do not, and every identifier pattern accepts
and refuses the values the crate's own validators accept and refuse
(crates/aegis-fabrica-defs/tests/bounded_fields.rs).

The validator here covers the keywords and the two combinator shapes schemars
writes for these types, and nothing else: a schema that grows another keyword
fails `test_the_schemas_use_only_the_keywords_this_reader_knows` by name
instead of being half-read. Python's `re` stands in for the ECMA-262 dialect
JSON Schema names; the patterns keep to classes, groups, alternation,
quantifiers and anchors, with no lookaround, which a sweep below holds them to.
One anchor still reads differently: Python's `$` also matches just before a
final newline, where ECMA-262's does not, so this reader matches with
`re.fullmatch`, which must consume the whole value as an ECMA-262 `^...$` does,
and every accepted value with a newline appended is refused. Payloads are read
as bytes, so no platform's line ending or encoding reaches the check (HISS-21).
"""

import itertools
import json
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BUILD = ROOT / "build"
KERNEL_SCHEMA = BUILD / "kernel-requirement.schema.json"
PRODUCT_SCHEMA = BUILD / "product-input.schema.json"
# Scalar bound on the (schema, value) pairs one validation visits (HISS-02).
MAX_STEPS = 8192
KNOWN_KEYWORDS = {
    "$schema",
    "$defs",
    "$ref",
    "title",
    "description",
    "default",
    "type",
    "format",
    "properties",
    "required",
    "additionalProperties",
    "items",
    "minItems",
    "maxItems",
    "pattern",
    "maxLength",
    "minimum",
    "maximum",
    "const",
    "oneOf",
    "anyOf",
}
JSON_TYPES = {
    "object": lambda value: isinstance(value, dict),
    "array": lambda value: isinstance(value, list),
    "string": lambda value: isinstance(value, str),
    "integer": lambda value: isinstance(value, int) and not isinstance(value, bool),
    "null": lambda value: value is None,
}
LOOKAROUND = re.compile(r"\(\?[=!<]")
# nucleus's own `required-by` rule, scripts/verify_kernel_requirement.py line 106
# at 82aa6b7a3c68a42a6330370c81ec642482014c9f, unchanged since 0a4eac9 (line 90):
# its ADR-0007 form (D103).
NUCLEUS_REQUIRED_BY = re.compile(r"(?!REQ-$)[A-Z][A-Z0-9-]*")
# The characters, and the longest value, the D103 equivalence sweep enumerates.
SWEEP_ALPHABET = "REQ-AF0a"
SWEEP_LENGTH = 6


def load(path):
    """Return the JSON document at `path`, read as bytes."""
    return json.loads(Path(path).read_bytes().decode("utf-8"))


def resolved(root, schema):
    """Return `schema` with a `#/$defs/<name>` reference replaced by the definition."""
    reference = schema.get("$ref")
    if reference is None:
        return schema
    name = reference.removeprefix("#/$defs/")
    siblings = {key: value for key, value in schema.items() if key != "$ref"}
    return dict(root["$defs"][name], **siblings)


def type_errors(schema, value, where):
    """Return why `value` is not of `schema`'s type or constant, or []."""
    types = schema.get("type")
    types = [types] if isinstance(types, str) else types or []
    if types and not any(JSON_TYPES[name](value) for name in types):
        return [f"{where}: {value!r} is not {'/'.join(types)}"]
    if "const" in schema and value != schema["const"]:
        return [f"{where}: {value!r} is not {schema['const']!r}"]
    return []


def object_errors(schema, value, where, pending):
    """Check one object's keys; queue each present property's value."""
    properties = schema.get("properties", {})
    errors = [f"{where}: no {key}" for key in schema.get("required", []) if key not in value]
    for key, item in value.items():
        if key in properties:
            pending.append((properties[key], item, f"{where}.{key}"))
        elif schema.get("additionalProperties") is False:
            errors.append(f"{where}: unknown field {key}")
    return errors


def array_errors(schema, value, where, pending):
    """Check one array's length; queue each item."""
    errors = []
    if len(value) < schema.get("minItems", 0) or len(value) > schema.get("maxItems", len(value)):
        errors.append(f"{where}: {len(value)} items is outside the bound")
    if "items" in schema:
        pending.extend(
            (schema["items"], item, f"{where}[{index}]") for index, item in enumerate(value)
        )
    return errors


def scalar_errors(schema, value, where):
    """Check one string's pattern and length, or one integer's range."""
    errors = []
    if isinstance(value, str):
        if "pattern" in schema and re.fullmatch(schema["pattern"], value) is None:
            errors.append(f"{where}: {value!r} does not match {schema['pattern']}")
        if len(value) > schema.get("maxLength", len(value)):
            errors.append(f"{where}: {len(value)} characters is past {schema['maxLength']}")
    elif isinstance(value, int):
        if not schema.get("minimum", value) <= value <= schema.get("maximum", value):
            errors.append(f"{where}: {value} is outside the range")
    return errors


def combinator_errors(schema, value, where, pending):
    """Read the two shapes schemars writes: oneOf of constants, anyOf of one schema and null."""
    if "oneOf" in schema:
        constants = [branch.get("const") for branch in schema["oneOf"]]
        return [] if value in constants else [f"{where}: {value!r} is not one of {constants}"]
    branches = schema.get("anyOf", [])
    if not branches or value is None and {"type": "null"} in branches:
        return []
    others = [branch for branch in branches if branch != {"type": "null"}]
    pending.extend((branch, value, where) for branch in others[:1])
    return [] if len(others) == 1 else [f"{where}: anyOf is not one schema or null"]


def validate(root, value):
    """Return every error of `value` against `root`, visiting at most MAX_STEPS pairs."""
    pending, errors = [(root, value, "$")], []
    for _ in range(MAX_STEPS):
        if not pending:
            return errors
        schema, item, where = pending.pop()
        schema = resolved(root, schema)
        errors += combinator_errors(schema, item, where, pending)
        errors += type_errors(schema, item, where) + scalar_errors(schema, item, where)
        if isinstance(item, dict) and "properties" in schema:
            errors += object_errors(schema, item, where, pending)
        elif isinstance(item, list):
            errors += array_errors(schema, item, where, pending)
    return errors + ([f"more than {MAX_STEPS} values"] if pending else [])


def keywords(schema):
    """Return every keyword `schema` uses, at any depth, walking a bounded worklist."""
    found, pending = set(), [schema]
    for _ in range(MAX_STEPS):
        if not pending:
            break
        node = pending.pop()
        if isinstance(node, list):
            pending.extend(node)
        elif isinstance(node, dict):
            found |= set(node)
            nested = [node.get("properties", {}), node.get("$defs", {})]
            pending.extend(value for group in nested for value in group.values())
            pending.extend(node.get(key) for key in ("items", "oneOf", "anyOf") if key in node)
    return found


def with_value(document, path, value):
    """Return a copy of `document` with the value at the key path `path` replaced."""
    copy = json.loads(json.dumps(document))
    target = copy
    for key in path[:-1]:
        target = target[key]
    target[path[-1]] = value
    return copy


class PublishedPayloadTests(unittest.TestCase):
    """The payloads Aegis publishes against the schemas it publishes."""

    kernel = load(KERNEL_SCHEMA)
    product = load(PRODUCT_SCHEMA)

    def test_every_published_payload_validates(self):
        for schema, name in (
            (self.kernel, "kernel-requirement.json"),
            (self.kernel, "kernel-requirement.reference.json"),
            (self.product, "product-input.json"),
        ):
            with self.subTest(payload=name):
                self.assertEqual(validate(schema, load(BUILD / name)), [])

    def test_a_producers_own_identifiers_validate(self):
        """Positive (D103): imago's flavour identifiers are admitted under the schema id."""
        document = load(BUILD / "kernel-requirement.json")
        for index, flavour in enumerate(("FLAVOR-BASE", "FLAVOR-K8S-NODE")):
            document = with_value(document, ["features", index, "required-by"], flavour)
        self.assertEqual(validate(self.kernel, document), [])

    def test_a_trailing_newline_is_refused(self):
        """Negative: a value an ECMA-262 `$` refuses, and the crate's charset too."""
        document = load(BUILD / "kernel-requirement.json")
        cases = (
            with_value(document, ["features", 0, "required-by"], "REQ-P07-01\n"),
            with_value(document, ["correlation-id"], document["correlation-id"] + "\n"),
        )
        for case in cases:
            with self.subTest(case=json.dumps(case)[:60]):
                self.assertEqual(len(validate(self.kernel, case)), 1)
        product = load(BUILD / "product-input.json")
        case = with_value(product, ["packages", 0], product["packages"][0] + "\n")
        self.assertEqual(len(validate(self.product, case)), 1)

    def test_the_array_form_and_an_unknown_field_are_refused(self):
        """Negative: what the decoder refuses, the schema refuses: at the top and nested."""
        document = load(BUILD / "kernel-requirement.json")
        cases = (
            list(document.values()),
            with_value(document, ["abi"], ["6.12", "7.3"]),
            with_value(document, ["features", 0], ["CONFIG_PREEMPT_RT", "built-in"]),
            dict(document, patches=[]),
            with_value(document, ["architectures"], ["x86_64"]),
            with_value(document, ["features", 0, "required-by"], "REQ-"),
        )
        for case in cases:
            with self.subTest(case=json.dumps(case)[:60]):
                self.assertTrue(validate(self.kernel, case))
        product = load(BUILD / "product-input.json")
        self.assertTrue(validate(self.product, with_value(product, ["retries"], [3, 30])))
        self.assertTrue(validate(self.product, dict(product, **{"image-digest": "x"})))

    def test_a_missing_field_or_an_empty_list_is_refused(self):
        document = load(BUILD / "kernel-requirement.json")
        del document["abi"]
        self.assertEqual(validate(self.kernel, document), ["$: no abi"])
        empty = with_value(load(BUILD / "kernel-requirement.json"), ["features"], [])
        self.assertEqual(len(validate(self.kernel, empty)), 1)

    def test_the_bounds_are_exact(self):
        """Boundary: each bound accepts its own value and refuses one past it."""
        document = load(BUILD / "kernel-requirement.json")
        row = document["features"][0]
        for count, refused in ((64, False), (65, True)):
            features = [dict(row, symbol=f"CONFIG_B{index}") for index in range(count)]
            self.assertEqual(
                bool(validate(self.kernel, dict(document, features=features))), refused
            )
        for length, refused in ((128, False), (129, True)):
            case = with_value(document, ["features", 0, "required-by"], "A" * length)
            self.assertEqual(bool(validate(self.kernel, case)), refused)
        product = load(BUILD / "product-input.json")
        for attempts, refused in ((1, False), (5, False), (0, True), (6, True)):
            case = with_value(product, ["retries", "max-attempts"], attempts)
            self.assertEqual(bool(validate(self.product, case)), refused, attempts)


# Each pattern, where it sits, and the values the crate's validators accept and refuse.
PATTERN_VECTORS = (
    (
        "kernel",
        "/properties/correlation-id",
        ["aegis-m18-product-input-0001", "a:b.c_d-e"],
        ["has space", ""],
    ),
    (
        "product",
        "/properties/revision",
        ["a" * 40, "0" * 40],
        ["a" * 12, "main", "HEAD", "A" * 40, "0" * 41],
    ),
    (
        "product",
        "/$defs/DistributionPin/properties/snapshot",
        ["2026/09/13"],
        ["latest", "rolling", "2026-09-13", "2026/9/13", "2026/09/13/1"],
    ),
    (
        "product",
        "/$defs/DefinitionReferences/properties/repart",
        ["build/repart.d", "build/mkosi.conf", ".", "...", ".hidden/x", "a/..b"],
        ["/etc/passwd", "../../secret", "build//repart.d", "a/../b", "build\\repart.d", "a/", ".."],
    ),
    (
        "product",
        "/properties/packages/items",
        ["linux-rt", "systemd-ukify", "0ad", "gtk+3"],
        ["Linux-RT", "-leading-dash", "@scope"],
    ),
    ("kernel", "/$defs/ArtifactExpectation/properties/digest", ["b" * 64], ["zz", "b" * 63]),
    (
        "kernel",
        "/$defs/ArtifactExpectation/properties/signature",
        ["deadbeef", "0" * 256],
        ["abc", "DEADBEEF", ""],
    ),
    (
        "kernel",
        "/$defs/FeatureRequirement/properties/symbol",
        ["CONFIG_BPF_LSM", "CONFIG_HZ_1000"],
        ["BPF_LSM", "CONFIG_", "config_bpf_lsm"],
    ),
    (
        "kernel",
        "/$defs/KernelAbi/properties/minimum-release",
        ["6.12", "7.2.4-1-cachyos", "7.3-rc2"],
        ["v7.3", "", "7 3"],
    ),
    (
        "kernel",
        "/$defs/FeatureRequirement/properties/required-by",
        ["REQ-P07-01", "FLAVOR-BASE", "FLAVOR-K8S-NODE", "A", "REQ-X", "FLAVOR-", "REQ-P07-"],
        ["7-P01", "-REQ-P07-01", "REQ-", "req-p07-01", ""],
    ),
)


class PatternTests(unittest.TestCase):
    """Every identifier pattern agrees with the validator it restates."""

    schemas = {"kernel": load(KERNEL_SCHEMA), "product": load(PRODUCT_SCHEMA)}

    def pattern(self, schema, pointer):
        node = self.schemas[schema]
        for key in pointer.strip("/").split("/"):
            node = node[key]
        return node["pattern"]

    def test_each_pattern_accepts_and_refuses_what_the_crate_does(self):
        """Every accepted value with a newline appended is refused as well: Python's `$`
        alone would admit it, an ECMA-262 `$` does not."""
        for schema, pointer, accepted, refused in PATTERN_VECTORS:
            pattern = self.pattern(schema, pointer)
            for value in accepted:
                with self.subTest(pointer=pointer, accepted=value):
                    self.assertIsNotNone(re.fullmatch(pattern, value))
            for value in refused + [value + "\n" for value in accepted]:
                with self.subTest(pointer=pointer, refused=value):
                    self.assertIsNone(re.fullmatch(pattern, value))

    def test_required_by_is_exactly_nucleus_adr_0007_form(self):
        """Boundary (D103): the lookaround-free pattern admits exactly what nucleus's
        `(?!REQ-$)[A-Z][A-Z0-9-]*` admits, over every value of up to SWEEP_LENGTH
        characters drawn from SWEEP_ALPHABET: the bare `REQ-` alone is refused."""
        pattern = self.pattern("kernel", "/$defs/FeatureRequirement/properties/required-by")
        differ, swept = [], 0
        for length in range(SWEEP_LENGTH + 1):
            for letters in itertools.product(SWEEP_ALPHABET, repeat=length):
                value = "".join(letters)
                swept += 1
                ours = re.fullmatch(pattern, value) is not None
                if ours != (NUCLEUS_REQUIRED_BY.fullmatch(value) is not None):
                    differ.append(value)
        self.assertEqual(differ, [])
        self.assertEqual(swept, sum(len(SWEEP_ALPHABET) ** n for n in range(SWEEP_LENGTH + 1)))
        self.assertIsNone(re.fullmatch(pattern, "REQ-"))
        self.assertIsNotNone(re.fullmatch(pattern, "REQ-P07-"))

    def test_every_pattern_is_anchored_and_free_of_lookaround(self):
        """Boundary: a pattern JSON Schema does not anchor for us, and one another dialect
        would read differently, are both refused."""
        found = 0
        for schema in self.schemas.values():
            pending = [schema]
            for _ in range(MAX_STEPS):
                if not pending:
                    break
                node = pending.pop()
                if isinstance(node, dict):
                    pattern = node.get("pattern")
                    found += pattern is not None
                    self.assertTrue(pattern is None or pattern.startswith("^"), pattern)
                    self.assertTrue(pattern is None or pattern.endswith("$"), pattern)
                    self.assertIsNone(LOOKAROUND.search(pattern or ""), pattern)
                    pending.extend(node.values())
                elif isinstance(node, list):
                    pending.extend(node)
        self.assertGreaterEqual(found, 14)

    def test_the_schemas_use_only_the_keywords_this_reader_knows(self):
        for name, schema in self.schemas.items():
            with self.subTest(schema=name):
                self.assertLessEqual(keywords(schema), KNOWN_KEYWORDS)
        self.assertFalse(keywords({"type": "object", "patternProperties": {}}) <= KNOWN_KEYWORDS)

    def test_the_walk_fails_closed_past_its_bound(self):
        """Boundary: a value too large to visit is an error, never a silent pass."""
        schema = {"type": "array", "items": {"type": "integer"}}
        self.assertEqual(validate(schema, [0] * (MAX_STEPS - 1)), [])
        self.assertEqual(validate(schema, [0] * MAX_STEPS), [f"more than {MAX_STEPS} values"])


if __name__ == "__main__":
    unittest.main()
