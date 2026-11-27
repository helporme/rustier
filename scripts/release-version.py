"""Set one release version in the tracked Cargo and npm packages. Never commit or tag."""

import argparse
import json
from pathlib import Path
import re
import subprocess
import tempfile


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("version", nargs="?", help="patch, minor, major, or an exact X.Y.Z[-prerelease]")
parser.add_argument("--check", action="store_true", help="verify without writing")
args = parser.parse_args()
if args.check == bool(args.version):
    parser.error("pass either a version or --check")

plugin = Path("packages/rustier/package.json")
current = json.loads(plugin.read_text())["version"]
version = current
if args.version:
    # Use npm's own SemVer rules, without allowing it to commit/tag a partial bump.
    increments = ("patch", "minor", "major")
    if args.version not in increments and not re.fullmatch(r"[0-9][0-9A-Za-z.-]*", args.version):
        parser.error("expected patch, minor, major, or an exact X.Y.Z[-prerelease]")
    with tempfile.TemporaryDirectory() as tmp:
        manifest = Path(tmp, "package.json")
        manifest.write_text(json.dumps({"name": "release-version", "version": current}))
        subprocess.run(["npm", "version", args.version, "--allow-same-version", "--no-git-tag-version",
                        "--ignore-scripts"], cwd=tmp, check=True, stdout=subprocess.DEVNULL)
        version = json.loads(manifest.read_text())["version"]
        if args.version not in increments and args.version != version:
            parser.error(f"use the canonical version {version}")

paths = subprocess.check_output(["git", "ls-files", "-z", "*Cargo.toml", "*package.json"])
manifests = [Path(p.decode()) for p in paths.split(b"\0") if p]
changes = {}


def update(path, source, result):
    if result != source:
        changes[path] = result


for path in manifests:
    source = path.read_text()
    if path.name == "package.json":
        result = re.sub(r'(?m)^([ \t]*"version": )"[^"]+"', rf'\g<1>"{version}"', source, count=1)
        if json.loads(result).get("version") != version:
            raise SystemExit(f"expected a top-level version in {path}")
        if path == plugin:
            result = re.sub(r'("rustier-bindings-unknown-unknown-unknown": )"[^"]+"', rf'\g<1>"{version}"', result)
    else:
        # Only [package].version and version requirements on local paths, never registry dependencies.
        result, count = re.subn(r'(?m)^(\[package\]\n(?:[^\[\n][^\n]*\n)*?version = )"[^"]+"',
                                rf'\g<1>"{version}"', source, count=1)
        if not count:
            raise SystemExit(f"no [package].version in {path}")
        result = re.sub(r'(?m)^([^#\n]+ = \{(?=[^}\n]*\bpath = )[^}\n]*\bversion = )"[^"]+"',
                        rf'\g<1>"={version}"', result)
    update(path, source, result)

# Clean checkouts have no generated npm packages yet. Keep their workspace link
# specifiers in sync without asking pnpm to resolve an as-yet unpublished version.
lock = Path("pnpm-lock.yaml")
source = lock.read_text()
result = re.sub(r'(?m)^(      rustier-bindings-[\w-]+:\n        specifier: )\S+(?=\n        version: link:)',
                rf'\g<1>{version}', source)
update(lock, source, result)
workspace = Path("pnpm-workspace.yaml")
source = workspace.read_text()
update(workspace, source, re.sub(r'(?m)^(  - rustier-bindings-[\w-]+@)\S+$', rf'\g<1>{version}', source))

if args.check and changes:
    raise SystemExit(f"expected {version} in: " + ", ".join(map(str, changes)))
for path, result in changes.items():
    path.write_text(result)

# Let Cargo resolve each independent lockfile (including examples and test crates).
for path in manifests:
    if path.name == "Cargo.toml":
        subprocess.run(["cargo", "tree", "--depth", "0", "--manifest-path", str(path),
                        *(["--locked"] if args.check else [])], check=True, stdout=subprocess.DEVNULL)
print(f"Rustier release version: {version}")
