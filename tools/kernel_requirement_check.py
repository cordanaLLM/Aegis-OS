#!/usr/bin/env python3
"""Check the Aegis kernel requirement against one kernel's own configuration (D94).

Milestone M10, epic E10-5. `build/kernel-requirement.json` is the M18 payload
Aegis hands to whoever builds the kernel. This tool reads it and a kernel
configuration -- in M10 the text the Nucleus kernel printed from its own
`/proc/config.gz` inside the guest -- and decides every feature row by the
rules `crates/aegis-fabrica-defs/src/kernel.rs` states for `RequiredState`:

* ``built-in`` is satisfied by ``=y`` only; ``=m`` does not satisfy it;
* ``module`` is satisfied by ``=m`` only; ``=y`` does not satisfy it;
* ``present`` is satisfied by ``=y`` or ``=m``;
* ``absent`` is satisfied by ``# CONFIG_X is not set`` or ``=n``.

It reimplements the configuration half of `KernelRequirement::unmet` and
fails closed where that does. A symbol the configuration does not mention at
all is unobserved, and an unobserved symbol satisfies no row, ``absent``
included: an unanswered question is not a measured absence. A value that is
not a tristate (a string or a number) satisfies no row either. Every listed
architecture must be the configuration's own (D104). When a release is given,
it must be at or above ``abi.minimum-release`` under the numeric rule of
`KernelRelease::at_least`, and equal to ``abi.module-abi`` when the payload
fixes one, as `unmet_identity` requires; without a release neither is
checked. A key the M18 schema does not name, at the top level, in ``abi`` or
in a feature row, is refused, as the Rust decoders' ``deny_unknown_fields``
refuses it.

It does not decide what the Rust half decides from a profile's capability
rows (`Unmet::CapabilityAbsent`), and it does not validate the optional
``artifact`` object beyond admitting its key.

Every row is recorded with the state the configuration gave it, satisfied or
not. A row that is not satisfied is rejected with the requirement's
correlation id and the symbol named, so the rejection can be threaded back to
the payload that asked for it.

The runtime probes a row names (the LSM list, the BTF blob, the powercap and
IOMMU trees) are M10's guest readings, recorded beside this decision by
`tools/verify_nucleus_kernel.py`; this tool decides the configuration only.
"""

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REQUIREMENT = ROOT / "build" / "kernel-requirement.json"
SCHEMA = "aegis.p01-nucleus.kernel-requirement.v1"
STATES = ("built-in", "module", "present", "absent")
PROBES = ("kernel-config", "lsm-list", "btf-vmlinux", "powercap", "iommu-groups")
# The configuration's own architecture symbol per listed spelling (mkosi's).
ARCHITECTURE_SYMBOLS = {"x86-64": "CONFIG_X86_64", "arm64": "CONFIG_ARM64"}
# The keys each object of the M18 schema admits; anything else is refused, as
# `deny_unknown_fields` refuses it in crates/aegis-fabrica-defs/src/kernel.rs.
TOP_KEYS = ("schema", "correlation-id", "architectures", "abi", "features", "artifact")
ABI_KEYS = ("minimum-release", "target-release", "module-abi")
FEATURE_KEYS = ("symbol", "state", "probe", "required-by")

# Scalar bounds (HISS-02), the schema's own where it states one.
MAX_FEATURES = 64
MAX_ARCHITECTURES = 4
MAX_CONFIG_LINES = 65536
MAX_CONFIG_BYTES = 4 << 20
MAX_RELEASE_BYTES = 64
MAX_RELEASE_COMPONENTS = 8

ASSIGNMENT = re.compile(r"^(CONFIG_[A-Za-z0-9_]+)=(.*)$")
NOT_SET = re.compile(r"^# (CONFIG_[A-Za-z0-9_]+) is not set$")
SYMBOL = re.compile(r"^CONFIG_[A-Z0-9_]+$")
CORRELATION = re.compile(r"^[A-Za-z0-9._:-]{1,128}$")
TRISTATE = {"y": "y", "m": "m", "n": "not set"}


class RequirementError(Exception):
    """The requirement payload cannot be read as the M18 schema states it."""


def parse_config(text):
    """Return ``{symbol: value}`` for one Kconfig configuration text.

    ``# CONFIG_X is not set`` reads as ``"not set"``; ``CONFIG_X=value`` reads
    as the value verbatim. A later line for the same symbol wins, which is how
    Kconfig merges fragments. Anything else is a comment and is skipped.
    """
    if len(text) > MAX_CONFIG_BYTES:
        raise RequirementError(f"the configuration is longer than {MAX_CONFIG_BYTES} bytes")
    found = {}
    for line in text.splitlines()[:MAX_CONFIG_LINES]:
        assignment = ASSIGNMENT.match(line)
        if assignment is not None:
            found[assignment.group(1)] = assignment.group(2)
            continue
        unset = NOT_SET.match(line)
        if unset is not None:
            found[unset.group(1)] = "not set"
    return found


def observed_state(config, symbol):
    """Return ``"y"``, ``"m"``, ``"not set"``, or None when unobserved or not a tristate."""
    value = config.get(symbol)
    if value is None:
        return None
    if value == "not set":
        return value
    return TRISTATE.get(value)


def satisfied(required, observed):
    """Return True when `observed` satisfies a row requiring `required`.

    None -- unobserved, or a value that is not a tristate -- satisfies nothing.
    """
    if observed is None:
        return False
    rules = {
        "built-in": observed == "y",
        "module": observed == "m",
        "present": observed in ("y", "m"),
        "absent": observed == "not set",
    }
    return rules[required]


def unknown_keys(label, value, allowed):
    """Return one problem per key of the object `value` that `allowed` does not name."""
    if not isinstance(value, dict):
        return []
    return [f"{label} carries the unknown key {key!r}" for key in value if key not in allowed]


def feature_problems(index, feature):
    """Return why one feature row is not a row the M18 schema admits, or []."""
    if not isinstance(feature, dict):
        return [f"features[{index}] is not an object"]
    problems = unknown_keys(f"features[{index}]", feature, FEATURE_KEYS)
    if not SYMBOL.match(str(feature.get("symbol", ""))):
        problems.append(f"features[{index}].symbol {feature.get('symbol')!r} is not CONFIG_*")
    if feature.get("state") not in STATES:
        problems.append(f"features[{index}].state {feature.get('state')!r} is not one of {STATES}")
    if feature.get("probe") not in PROBES:
        problems.append(f"features[{index}].probe {feature.get('probe')!r} is not one of {PROBES}")
    if not feature.get("required-by"):
        problems.append(f"features[{index}] names no required-by")
    return problems


def bounded_list(value, bound):
    """Return True when `value` is a list of 1..`bound` entries."""
    return isinstance(value, list) and 1 <= len(value) <= bound


def requirement_problems(payload):
    """Return why `payload` is not an M18 kernel requirement this tool can decide, or []."""
    if not isinstance(payload, dict):
        return ["the requirement is not a JSON object"]
    problems = []
    if payload.get("schema") != SCHEMA:
        problems.append(f"schema is {payload.get('schema')!r}, not {SCHEMA!r}")
    if not CORRELATION.match(str(payload.get("correlation-id", ""))):
        problems.append("correlation-id is absent or outside [A-Za-z0-9._:-]{1,128}")
    features = payload.get("features")
    if not bounded_list(features, MAX_FEATURES):
        problems.append(f"features must list 1..{MAX_FEATURES} rows")
        features = []
    for index, feature in enumerate(features[:MAX_FEATURES]):
        problems.extend(feature_problems(index, feature))
    if not bounded_list(payload.get("architectures"), MAX_ARCHITECTURES):
        problems.append(f"architectures must list 1..{MAX_ARCHITECTURES} entries")
    return problems + abi_problems(payload) + unknown_keys("the requirement", payload, TOP_KEYS)


def abi_problems(payload):
    """Return why the payload's `abi` object is not one the M18 schema admits, or []."""
    abi = payload.get("abi")
    if not isinstance(abi, dict):
        return ["abi is absent or not an object"]
    problems = unknown_keys("abi", abi, ABI_KEYS)
    minimum = abi.get("minimum-release")
    if not isinstance(minimum, str) or not minimum[:1].isdigit():
        problems.append("abi.minimum-release is absent or does not start with a digit")
    module_abi = abi.get("module-abi")
    if module_abi is not None and (not isinstance(module_abi, str) or not module_abi):
        problems.append("abi.module-abi is present and is not a non-empty string")
    return problems


def load_requirement(path=REQUIREMENT):
    """Return the parsed requirement, or raise RequirementError naming every problem."""
    try:
        payload = json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        raise RequirementError(f"{path} could not be read as JSON: {error}") from error
    problems = requirement_problems(payload)
    if problems:
        raise RequirementError("; ".join(problems))
    return payload


def numeric_components(text):
    """Return the leading dotted numeric components of a release, as `field.rs` reads them."""
    out = []
    current = None
    for character in text[:MAX_RELEASE_BYTES]:
        if character.isdigit():
            current = (current or 0) * 10 + int(character)
            continue
        if character != "." or current is None or len(out) >= MAX_RELEASE_COMPONENTS:
            break
        out.append(current)
        current = None
    if current is not None and len(out) < MAX_RELEASE_COMPONENTS:
        out.append(current)
    return out


def release_at_least(release, minimum):
    """Return True when `release` is at or above `minimum` (`KernelRelease::at_least`).

    Components are compared numerically, a missing component reads as 0, and
    the first difference decides: 6.12 admits 6.12 and 6.12.0-rt, and refuses
    6.11.99.
    """
    mine = numeric_components(release)
    floor = numeric_components(minimum)
    for index in range(MAX_RELEASE_COMPONENTS):
        left = mine[index] if index < len(mine) else 0
        right = floor[index] if index < len(floor) else 0
        if left != right:
            return left > right
    return True


def config_architecture(config):
    """Return the mkosi spelling of the configuration's own architecture, or None."""
    for spelling, symbol in ARCHITECTURE_SYMBOLS.items():
        if observed_state(config, symbol) == "y":
            return spelling
    return None


def rows(requirement, config):
    """Return one decided row per feature: symbol, required, observed, satisfied."""
    decided = []
    for feature in requirement["features"][:MAX_FEATURES]:
        observed = observed_state(config, feature["symbol"])
        decided.append(
            {
                "symbol": feature["symbol"],
                "required": feature["state"],
                "observed": observed if observed is not None else "unobserved",
                "probe": feature["probe"],
                "required-by": feature["required-by"],
                "satisfied": satisfied(feature["state"], observed),
            }
        )
    return decided


def identity_rejections(requirement, config, release):
    """Return the architecture, release and module-ABI rejections, each naming the id."""
    correlation = requirement["correlation-id"]
    found = config_architecture(config)
    rejected = []
    for architecture in list(dict.fromkeys(requirement["architectures"]))[:MAX_ARCHITECTURES]:
        if architecture != found:
            rejected.append(
                f"rejected: correlation-id {correlation}: architecture {architecture} is "
                f"listed and the configuration is for {found or 'no listed architecture'}"
            )
    return rejected + release_rejections(requirement, release)


def release_rejections(requirement, release):
    """Return the release-floor and module-ABI rejections; none when no release is given."""
    if release is None:
        return []
    correlation = requirement["correlation-id"]
    minimum = requirement["abi"]["minimum-release"]
    rejected = []
    if not release_at_least(release, minimum):
        rejected.append(
            f"rejected: correlation-id {correlation}: kernel release {release} is below "
            f"abi.minimum-release {minimum}"
        )
    module_abi = requirement["abi"].get("module-abi")
    if module_abi is not None and release != module_abi:
        rejected.append(
            f"rejected: correlation-id {correlation}: kernel release {release} is not "
            f"abi.module-abi {module_abi}"
        )
    return rejected


def rejections(requirement, decided):
    """Return one line per unsatisfied row, naming the correlation id and the symbol."""
    correlation = requirement["correlation-id"]
    return [
        f"rejected: correlation-id {correlation}: {row['symbol']} is required {row['required']} "
        f"({row['required-by']}) and the configuration has {row['observed']}"
        for row in decided
        if not row["satisfied"]
    ]


def check(requirement, config_text, release=None):
    """Decide `requirement` against one configuration text.

    Returns ``(rows, rejections)``; the kernel satisfies the requirement when
    the second is empty.
    """
    config = parse_config(config_text)
    decided = rows(requirement, config)
    rejected = identity_rejections(requirement, config, release) + rejections(requirement, decided)
    return decided, rejected


def row_line(row):
    """Return the printable record of one decided row."""
    verdict = "satisfied" if row["satisfied"] else "NOT satisfied"
    return (
        f"{row['symbol']}: required {row['required']}, config {row['observed']} -> {verdict} "
        f"({row['required-by']}, probe {row['probe']})"
    )


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--config", required=True, help="a kernel configuration text")
    parser.add_argument("--requirement", default=str(REQUIREMENT))
    parser.add_argument("--release", help="the kernel release, checked against the ABI floor")
    args = parser.parse_args(argv)
    try:
        requirement = load_requirement(args.requirement)
        text = Path(args.config).read_text(encoding="utf-8", errors="replace")
        decided, rejected = check(requirement, text, args.release)
    except (RequirementError, OSError) as error:
        print(f"FAIL: the requirement could not be checked: {error}")
        return 2
    for row in decided:
        print(row_line(row))
    for line in rejected:
        print(line)
    if rejected:
        return 1
    print(
        f"PASS: correlation-id {requirement['correlation-id']}: {len(decided)} features satisfied"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
