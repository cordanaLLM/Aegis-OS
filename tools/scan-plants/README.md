# Planted defects for the security scan steps

The Verification gate (`.github/workflows/ci.yml`) copies these files into a
scratch directory, drops the `.plant` suffix and runs each scanner on them
before it scans the repository. The step fails unless the scanner refuses every
plant, so a scanner that stopped detecting cannot pass the gate silently.

| File | Scanner | Planted defect |
| :--- | :--- | :--- |
| `dynamic.py.plant` | semgrep | every Python rule in `.config/semgrep/hiss.yml` |
| `dynamic.js.plant` | semgrep | eval and the Function constructor |
| `banned.c.plant` | semgrep | strcpy and sprintf |
| `workflow.yml.plant` | semgrep | a pull request title expanded inside `run:` |
| `requirements.txt.plant` | trivy | a Django release with fixed HIGH and CRITICAL CVEs |

The `.plant` suffix keeps the files out of the repository's own scans and lints.
The gitleaks plant is generated at run time: a committed token would itself be
a finding.
