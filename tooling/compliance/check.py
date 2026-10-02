"""Dependency/notice drift guard, not a legal audit or release approval.

Python 3.11+, standard library only. Never builds or runs dependency code.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib


MANIFESTS = {
    "Cargo.toml", "Cargo.lock", "package.json", "package-lock.json",
    "npm-shrinkwrap.json", "pnpm-lock.yaml", "yarn.lock", "bun.lock", "bun.lockb",
    "go.mod", "go.sum", "pyproject.toml", "poetry.lock", "uv.lock", "Pipfile",
    "Pipfile.lock", "requirements.txt", "pom.xml", "build.gradle", "build.gradle.kts",
    "gradle.lockfile", "Gemfile", "Gemfile.lock", "composer.json", "composer.lock",
    "packages.lock.json", "Directory.Packages.props", "flake.lock", "flake.nix",
    "mise.toml", "mise.lock", ".gitmodules", "vcpkg.json", "vcpkg-configuration.json",
}
NOTICE = re.compile(r"^(license|licence|copying|copyright|notice|third.party.notices)([._-].*)?$", re.I)


def git(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args])


def kind(path):
    name = Path(path).name
    if NOTICE.match(name):
        return "notice"
    if (name in MANIFESTS or re.fullmatch(r"requirements.*\.txt", name)
            or name.endswith((".csproj", ".fsproj", ".vbproj", ".gemspec"))):
        return "dependency-input"
    return None


def digest(data):
    return hashlib.sha256(data.replace(b"\r\n", b"\n")).hexdigest()


def read_json(path):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"duplicate JSON key: {key}")
            result[key] = value
        return result
    return json.loads(path.read_text(encoding="utf-8-sig"), object_pairs_hook=unique)


def snapshot(root):
    root = root.resolve()
    paths = git(root, "ls-files", "-z").decode().split("\0")
    inputs, packages = [], []
    for name in sorted(filter(None, paths)):
        category = kind(name)
        if not category:
            continue
        path = root / name
        if path.is_symlink() or not path.resolve().is_relative_to(root):
            raise ValueError(f"tracked compliance input is a link or escapes root: {name}")
        data = path.read_bytes()
        inputs.append({"path": name, "kind": category, "sha256_lf": digest(data)})
        if path.name == "Cargo.lock":
            for package in tomllib.loads(data.decode("utf-8-sig")).get("package", []):
                packages.append({"ecosystem": "cargo", "lockfile": name,
                                 "name": package["name"], "version": package["version"],
                                 "source": package.get("source", "workspace"),
                                 "checksum": package.get("checksum"),
                                 "license": None, "disposition": "unreviewed"})
        elif path.name in {"package-lock.json", "npm-shrinkwrap.json"}:
            lock = read_json(path)
            # Old lock formats remain hashed inputs, not a falsely complete inventory.
            for location, package in sorted(lock.get("packages", {}).items()):
                if not location or package.get("link"):
                    continue
                packages.append({"ecosystem": "npm", "lockfile": name,
                                 "name": package.get("name", location.split("node_modules/")[-1]),
                                 "version": package.get("version"), "location": location,
                                 "license": package.get("license"), "disposition": "unreviewed"})
    return {"schema": 1, "scope": "tracked input hashes plus Cargo and npm lock records; not a release SBOM",
            "inputs": inputs, "packages": packages}


def verify_components(root, document):
    if document.get("schema") != 1 or not isinstance(document.get("components"), list):
        raise ValueError("invalid component inventory")
    seen = set()
    for component in document["components"]:
        identity = component["id"]
        if identity in seen:
            raise ValueError(f"duplicate component: {identity}")
        seen.add(identity)
        if component.get("review_status") != "pending":
            raise ValueError("foundation supports pending reviews only; approval requires a reviewed implementation")
        if not component.get("origin") or not component.get("scope"):
            raise ValueError(f"missing provenance/scope: {identity}")
        for notice in component.get("notices", []):
            path = root / notice["path"]
            if path.is_symlink() or not path.resolve().is_relative_to(root.resolve()):
                raise ValueError("notice escapes repository or is a link")
            if digest(path.read_bytes()) != notice["sha256_lf"]:
                raise ValueError(f"notice mismatch: {notice['path']}")


def drift(before, after):
    a = {row["path"]: row for row in before["inputs"]}
    b = {row["path"]: row for row in after["inputs"]}
    return sorted(path for path in a.keys() | b.keys() if a.get(path) != b.get(path))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--write-baseline", action="store_true")
    parser.add_argument("--base", help="Git base ref for a review diff; does not approve it")
    parser.add_argument("--release", action="store_true", help="fail closed: release audit is not implemented")
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    root = args.root.resolve()
    policy = read_json(root / "compliance/policy.json")
    if (policy.get("schema") != 1 or policy.get("status") != "provisional"
            or policy.get("distribution") != "blocked_pending_audit"
            or policy.get("approved_license_expressions") != []):
        raise ValueError("foundation policy must remain provisional; no automated license approvals")
    current = snapshot(root)
    baseline_path = root / "compliance/baseline.json"
    if args.write_baseline:
        baseline_path.write_text(json.dumps(current, indent=2) + "\n", encoding="utf-8")
        print("Wrote factual baseline. ALL components remain unreviewed; this is not approval.")
        return 0
    baseline = read_json(baseline_path)
    if baseline != current:
        changed = drift(baseline, current)
        raise ValueError("inventory drift: " + ", ".join(changed or ["package records/schema changed"]) +
                         ". Investigate, preserve notices and regenerate factual baseline for human review.")
    verify_components(root, read_json(root / "compliance/components.json"))
    changes = []
    bootstrap = False
    if args.base:
        exists = subprocess.run(["git", "-C", str(root), "cat-file", "-e",
                                 f"{args.base}:compliance/baseline.json"], capture_output=True).returncode == 0
        if exists:
            previous = json.loads(git(root, "show", f"{args.base}:compliance/baseline.json"))
            changes = drift(previous, current)
        else:
            bootstrap = True
    report = {"status": "inventory-consistent", "legal_approval": False,
              "release_ready": False, "input_count": len(current["inputs"]),
              "lock_record_count": len(current["packages"]),
              "unreviewed_lock_records": len(current["packages"]),
              "changed_inputs_from_base": changes, "baseline_bootstrap": bootstrap,
              "review_required": bool(changes) or bootstrap,
              "limitations": ["No exact target/feature shipping graph or license-text audit",
                              "No code-copy detector or complete non-Cargo/npm dependency inventory",
                              "No artifact notice/source-availability validation yet"]}
    if args.report:
        args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    if args.release:
        print("RELEASE BLOCKED: license policy, target audit, notices and source obligations are not approved.", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        print(f"Compliance check failed: {error}", file=sys.stderr)
        sys.exit(1)
