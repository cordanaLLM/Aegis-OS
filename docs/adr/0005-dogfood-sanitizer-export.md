<!-- markdownlint-disable MD013 -->
# ADR-0005: Dogfood findings reach public repositories through one allow-list export

- **Status**: Accepted
- **Date**: 2026-10-17
- **Authors**: @lusoris (decision, paraphrased in the controller ledger as aegis-os-osbound-homezeus-changed-20261017-correction)

## Context

Aegis modules and gates already run on the workstation that serves as the
reference profile: the eBPF objects load through the host verifier (M19), the
RAPL and Firecracker slices run on its KVM (M21), and the display slice presents
frames on its Arc A380 (M27, M32). That workstation is managed from a private
configuration repository. It dogfoods Aegis modules the way a cluster
repository dogfoods them on a cluster: it runs them and reports what it found.

Aegis is not a copy of the workstation's distribution. Its host tuning, units
and kernel command line do not become Aegis configuration. What should reach
Aegis, or a module repository, is narrower: which module or gate ran, on which
milestone, with which verdict and which measurements.

A record written on the workstation can carry what must stay private: the host
name, addresses, MAC addresses, serial numbers, user names, e-mail addresses,
keys and tokens, home-folder paths, and the operator's own wording. A redactor
that replaces known patterns lets through every shape it did not foresee. The
workstation repository already runs one such redactor for memory traffic, and it
is the right tool there, because that traffic has no fixed shape. A dogfood
finding has one.

## Decision

1. A dogfood finding reaches a public Aegis or module repository only through
   `tools/dogfood_export.py`. No other path copies workstation output into a
   public tree.
2. The export is an allow-list. A record holds only the keys the tool declares
   (`schema`, `date`, `module`, `milestone`, `verdict`, `requirement`,
   `kernel_release`, `driver`, `pci_id`, `evidence_sha256`, `measurements`),
   and each value must match its declared shape. `module`, `milestone` and
   `requirement` must name something that exists in this repository; a
   measurement name must be one the tool lists, and its value a finite number.
   There is no free-text field, so no wording leaves the workstation.
3. The export refuses; it never redacts. One violation refuses the whole record,
   every reason is printed, and no output file is written.
4. A second, independent barrier scans the finished output for IPv4 and IPv6
   addresses, MAC addresses, e-mail addresses, home-folder paths, private-key
   and SSH-key markers, common token prefixes, the running host's name, and
   every name in a deny-hosts file. The deny-hosts file lives in the private
   workstation repository and is passed with `--deny-hosts`; it is never
   committed here. A refusal names the rule that matched, never the deny-hosts
   entry, and a host name that cannot be read refuses the record.
5. `tools/test_dogfood_export.py` runs inside `make verify-all`. It refuses a
   planted host name, IP address, e-mail address and key, and it holds a case
   that only the second barrier catches, so removing either barrier fails the
   suite.
6. The fleet module map records the workstation as a dogfood consumer of each
   Aegis module (relation `dogfoods`), with gap rows in both directions: Aegis
   modules the workstation does not run yet, and workstation functions with no
   Aegis module. A workstation function becomes an Aegis requirement through
   the requirement register, never by importing its configuration.

## Consequences

- **Positive**: one reviewed path with a fixed shape replaces ad-hoc copying.
  A leak needs a schema change in this repository, which goes through review,
  and must still pass the second scan.
- **Positive**: an adopter's own workstation repository can feed findings to
  its fork of Aegis the same way, with its own deny-hosts file.
- **Negative**: a new measurement or field needs a code change to the export
  before it can be published. That is the intended cost.
- **Negative**: the deny scan can refuse a legitimate value that looks like an
  address. The fix is a reviewed change to the allow-list shape, never a bypass.
