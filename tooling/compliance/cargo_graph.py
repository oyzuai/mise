"""Read-only candidate Cargo graph and notice evidence, never legal approval.

Uses offline Cargo metadata; does not build dependencies or select SPDX options.
The non-dev closure is review input, not proof of the eventual shipping graph.
"""
import argparse
from collections import Counter
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib
from urllib.parse import urlsplit

SPEC = importlib.util.spec_from_file_location("compliance_check", Path(__file__).with_name("check.py"))
guard = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(guard)
FEATURES = "rustls,vendored-lua"
TARGETS = ("x86_64-unknown-linux-gnu", "aarch64-apple-darwin", "x86_64-pc-windows-msvc")


def package_key(package, root):
    source = package.get("source")
    if source is None:
        relative = Path(package["manifest_path"]).resolve().relative_to(root)
        source = "workspace:" + relative.parent.as_posix()
    else:
        if not source.startswith(("registry+https://", "git+https://")):
            raise ValueError("unsupported Cargo source; manual source audit required")
        parsed = urlsplit(source.split("+", 1)[1])
        if parsed.username or parsed.password or any(ord(c) < 32 for c in source):
            raise ValueError("credentialed or malformed Cargo source")
        # Query strings can contain credentials. Do not publish them as evidence.
        if parsed.query:
            raise ValueError("queried Cargo source needs a separately reviewed identity")
    return f"{source}#{package['name']}@{package['version']}"


def notices(package):
    root = Path(package["manifest_path"]).resolve().parent
    found = []
    seen = 0
    skipped = 0
    def scan_error(error):
        # os.walk otherwise ignores unreadable directories. An incomplete scan
        # must never look like successful notice evidence. Do not relay paths.
        raise ValueError("package notice directory could not be read") from error

    for directory, dirs, files in os.walk(root, followlinks=False, onerror=scan_error):
        dirs[:] = sorted(d for d in dirs if d not in {".git", "target"})
        for name in list(dirs):
            child = Path(directory) / name
            if child.is_symlink() or (hasattr(child, "is_junction") and child.is_junction()):
                dirs.remove(name)
                skipped += 1
        seen += len(files) + len(dirs)
        if seen > 100_000:
            raise ValueError("package notice scan exceeds entry budget")
        for name in sorted(files):
            if not guard.NOTICE.match(name):
                continue
            path = Path(directory) / name
            if path.is_symlink() or not path.is_file():
                skipped += 1
                continue
            found.append({"path":path.relative_to(root).as_posix(), "sha256_lf":notice_digest(path)})
            if len(found) > 4096:
                raise ValueError("package notice count exceeds limit")
    declared = package.get("license_file")
    declared_status = "absent"
    if declared:
        path = root / declared
        try:
            relative = path.resolve(strict=True).relative_to(root)
        except (OSError, ValueError):
            declared_status = "unresolved-or-outside-package"
        else:
            if path.is_symlink() or not path.is_file():
                declared_status = "redirected-or-not-file"
            else:
                declared_status = "observed"
                if relative.as_posix() not in {n["path"] for n in found}:
                    found.append({"path":relative.as_posix(), "sha256_lf":notice_digest(path)})
    return sorted(found, key=lambda n:n["path"]), declared_status, skipped


def notice_digest(path):
    with path.open("rb") as file:
        data = file.read(2 * 1024 * 1024 + 1)
    if len(data) > 2 * 1024 * 1024:
        raise ValueError("notice exceeds evidence byte limit")
    return guard.digest(data)


def inventory(metadata, root, target, revision, lock_bytes, package_roots=None):
    root = root.resolve()
    packages = {p["id"]:p for p in metadata["packages"]}
    nodes = {n["id"]:n for n in metadata["resolve"]["nodes"]}
    if len(packages) != len(metadata["packages"]) or len(nodes) != len(metadata["resolve"]["nodes"]):
        raise ValueError("duplicate Cargo package/node identity")
    if len(packages) > 4096 or len(nodes) > 4096:
        raise ValueError("Cargo graph exceeds package budget")
    start = metadata["resolve"]["root"]
    if start not in packages or packages[start]["name"] != "mise":
        raise ValueError("metadata root must be the mise candidate")
    pending, selected, edges = [start], set(), {}
    while pending:
        key = pending.pop()
        if key in selected:
            continue
        selected.add(key)
        outgoing = []
        for dep in nodes[key]["deps"]:
            kinds = sorted({kind["kind"] or "normal" for kind in dep["dep_kinds"] if kind["kind"] in (None,"normal","build")})
            if kinds:
                outgoing.append((dep["pkg"], kinds))
                pending.append(dep["pkg"])
        edges[key] = outgoing
    identities = {key:package_key(packages[key], root) for key in selected}
    checksums = {(p["name"],p["version"],p.get("source")):p.get("checksum") for p in tomllib.loads(lock_bytes.decode())["package"]}
    records = []
    for key in sorted(selected, key=identities.get):
        package = packages[key]
        if package_roots is not None:
            package_roots[identities[key]] = Path(package["manifest_path"]).resolve().parent
        identity = (package["name"], package["version"], package.get("source"))
        if identity not in checksums:
            raise ValueError("selected package is absent from Cargo.lock")
        files, declared, skipped = notices(package)
        records.append({"id":identities[key], "name":package["name"], "version":package["version"],
            "declared_license":package.get("license"), "package_checksum":checksums[identity],
            "resolved_features":sorted(nodes[key]["features"]), "notice_files":files,
            "declared_license_file_status":declared, "skipped_links":skipped,
            "dependencies":sorted(({"id":identities[dep], "kinds":kinds} for dep,kinds in edges[key]),key=lambda d:d["id"]),
            "disposition":"unreviewed"})
    return {"format":1,"scope":"target-filtered-cargo-non-dev-closure", "source_revision":revision,
        "target":target,"default_features":False,"features":FEATURES.split(","),
        "cargo_lock_sha256_lf":guard.digest(lock_bytes),"metadata_sha256":hashlib.sha256(json.dumps(metadata,sort_keys=True).encode()).hexdigest(),
        "package_count":len(records),"declared_license_counts":dict(sorted(Counter(p["declared_license"] or "UNDECLARED" for p in records).items())),
        "packages":records,"legal_approval":False,"release_ready":False,
        "limitations":["Cargo metadata can unify features with development targets; this is not an exact shipping graph",
            "Build/proc-macro inputs remain review inputs; metadata does not prove which code ships",
            "Conventional notice filenames and declared license files only; source headers and generated/vendored obligations need human audit",
            "License expressions are declarations, not validated obligations or selected alternatives",
            "No artifact packaging, source-availability fulfillment or legal approval is performed"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--notice-bundle", type=Path)
    args = parser.parse_args()
    if args.notice_bundle and args.notice_bundle.resolve() == args.output.resolve():
        parser.error("notice bundle and report need different paths")
    root = Path(__file__).resolve().parents[2]
    revision = guard.git(root,"rev-parse","HEAD").decode().strip()
    before = (root/"Cargo.lock").read_bytes()
    with tempfile.TemporaryFile() as output:
        result = subprocess.run(["cargo","metadata","--locked","--offline","--format-version","1",
            "--filter-platform",args.target,"--no-default-features","--features",FEATURES],
            cwd=root,stdout=output,stderr=subprocess.DEVNULL,timeout=180)
        if result.returncode:
            raise RuntimeError("offline Cargo metadata failed; provision cached package metadata and the compiler separately")
        if output.tell() > 64 * 1024 * 1024:
            raise ValueError("Cargo metadata exceeds evidence budget")
        output.seek(0)
        metadata = json.load(output)
    if before != (root/"Cargo.lock").read_bytes():
        raise ValueError("Cargo.lock changed during evidence collection")
    roots = {}
    report = inventory(metadata,root,args.target,revision,before,package_roots=roots)
    report["tracked_worktree_changes"] = bool(guard.git(root,"diff","--name-only","HEAD"))
    report["collector_sha256_lf"] = guard.digest(Path(__file__).read_bytes())
    report["notice_guard_sha256_lf"] = guard.digest(Path(__file__).with_name("check.py").read_bytes())
    report["notice_bundle_collector_sha256_lf"] = guard.digest(Path(__file__).with_name("cargo_notices.py").read_bytes())
    report_bytes = (json.dumps(report,indent=2)+"\n").encode("utf-8")
    if args.notice_bundle:
        from cargo_notices import write_bundle
        write_bundle(args.notice_bundle, report, report_bytes, roots, guard.digest)
    args.output.write_bytes(report_bytes)
    print(json.dumps({"package_count":report["package_count"],"legal_approval":False,"release_ready":False}))


if __name__ == "__main__":
    main()
