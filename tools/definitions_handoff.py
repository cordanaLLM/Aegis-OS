#!/usr/bin/env python3
"""Hand a definition image a pull request built to the runs that cannot read its cache.

An Actions cache saved by a pull request run is scoped to that pull request's
merge ref: the run on main after the merge cannot restore it. When a pull
request changes a pin, the first main run therefore found no saved image, and
with the archive server down its fetch failed although the image had been
built and saved minutes before (run 39029749159).

So a run whose cache missed also uploads the image it built as an artifact
named after the cache key, and a run whose cache missed first looks for one:

* it lists the repository's artifacts of that name, keeps those that have not
  expired and whose run came from this repository itself, never a fork, and
  picks the newest;
* it downloads that artifact and extracts exactly one member, the archive
  `definitions_container.cache_archive()` names for the pin, into the cache
  directory, where the fetch finds it as its second source.

The handed-off image is a second source like a cached one: the fetch loads it
only after every attempt against the archive server failed, and checks it
against the pin the same way. Finding none is not an error; the fetch still
fails, and names both sources, when the server is unreachable too.
"""

import argparse
import json
import os
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

import definitions_container as container

API_TIMEOUT = 60
DOWNLOAD_TIMEOUT = 900
# One page of the artifact listing; GitHub serves at most 100 per page, and
# artifacts of one name beyond the newest hundred are never the newest one.
PER_PAGE = 100


def gh(argv, timeout, stdout=None):
    """Run `gh api` with a deadline; return (exit code, stdout text, stderr text)."""
    try:
        done = subprocess.run(
            ["gh", "api", *argv],
            stdout=stdout if stdout is not None else subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
            check=False,
        )
    except (subprocess.TimeoutExpired, OSError) as error:
        return 1, "", f"gh api did not complete: {error}"
    text = done.stdout.decode(errors="replace") if done.stdout is not None else ""
    return done.returncode, text, done.stderr.decode(errors="replace")


def newest(listing, name, repository_id):
    """Return the newest usable artifact called `name` in `listing`, or None."""
    usable = [
        artifact
        for artifact in listing.get("artifacts", [])[:PER_PAGE]
        if artifact.get("name") == name
        and artifact.get("expired") is False
        and (artifact.get("workflow_run") or {}).get("head_repository_id") == repository_id
        and (artifact.get("workflow_run") or {}).get("repository_id") == repository_id
    ]
    if not usable:
        return None
    return max(usable, key=lambda artifact: artifact.get("created_at", ""))


def extract(bundle, member, archive):
    """Write `member` of the zip file `bundle` to `archive`; return None or why not."""
    try:
        with zipfile.ZipFile(bundle) as unpacked:
            if member not in unpacked.namelist():
                return f"the artifact holds no {member}"
            archive.parent.mkdir(parents=True, exist_ok=True)
            with unpacked.open(member) as source, open(archive, "wb") as target:
                for block in iter(lambda: source.read(1 << 20), b""):
                    target.write(block)
    except (zipfile.BadZipFile, OSError) as error:
        return f"the artifact did not unpack: {error}"
    return None


def download(repository, artifact, member, archive, call=gh):
    """Download `artifact` and extract `member` to `archive`; return None or why not."""
    with tempfile.TemporaryDirectory() as scratch:
        bundle = Path(scratch) / "artifact.zip"
        with open(bundle, "wb") as sink:
            code, _, stderr = call(
                [f"repos/{repository}/actions/artifacts/{artifact['id']}/zip"],
                DOWNLOAD_TIMEOUT,
                stdout=sink,
            )
        if code != 0:
            return f"the download exited {code} {stderr}".strip()
        return extract(bundle, member, archive)


def handoff(name, path=container.PIN, environ=None, call=gh):
    """Place a handed-off image where the fetch looks; return a line saying what happened."""
    environ = os.environ if environ is None else environ
    _, raw = container.load_pin(path)
    archive = container.cache_archive(container.image_tag(raw), environ)
    if archive is None:
        return f"SKIP: {container.CACHE_VARIABLE} is not set, so there is nowhere to hand off to."
    if archive.is_file():
        return f"SKIP: {archive} is already present."
    repository = environ.get("GITHUB_REPOSITORY", "")
    repository_id = environ.get("GITHUB_REPOSITORY_ID", "")
    if not repository or not repository_id.isdigit():
        return "SKIP: GITHUB_REPOSITORY or GITHUB_REPOSITORY_ID is unset; not in a workflow run."
    code, listing, stderr = call(
        [f"repos/{repository}/actions/artifacts?name={name}&per_page={PER_PAGE}"], API_TIMEOUT
    )
    if code != 0:
        return f"SKIP: the artifact listing exited {code} {stderr}".strip()
    try:
        chosen = newest(json.loads(listing), name, int(repository_id))
    except (ValueError, AttributeError) as error:
        return f"SKIP: the artifact listing did not parse: {error}"
    if chosen is None:
        return f"SKIP: no unexpired artifact {name} from this repository."
    problem = download(repository, chosen, archive.name, archive, call)
    if problem is not None:
        archive.unlink(missing_ok=True)
        return f"SKIP: artifact {chosen['id']}: {problem}"
    run_id = chosen["workflow_run"].get("id")
    return f"PASS: {archive} handed off from artifact {chosen['id']} of run {run_id}."


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--name", required=True, help="the artifact name, the cache key")
    parser.add_argument(
        "--floor", action="store_true", help=f"hand off the image {container.FLOOR_PIN.name} pins"
    )
    options = parser.parse_args()
    print(handoff(options.name, container.FLOOR_PIN if options.floor else container.PIN))
    return 0


if __name__ == "__main__":
    sys.exit(main())
