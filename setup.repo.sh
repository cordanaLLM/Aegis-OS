#!/usr/bin/env bash
# Stage private proposals through Praetor; original export script is archived.
set -euo pipefail
repo_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
praetor_source="${PRAETOR_SOURCE_ROOT:-${repo_dir}/../praetor}"
exec python3 -B "${praetor_source}/scripts/planning_import.py" \
    --manifest "${repo_dir}/.workingdir/import-manifest.json" \
    --source "${repo_dir}/.workingdir/notebookllmprep" \
    --output "${repo_dir}/.workingdir/prepared" "$@"
