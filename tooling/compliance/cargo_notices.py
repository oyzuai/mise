"""Preserve observed Cargo notices as review evidence, never license approval."""
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import zipfile

MAX_FILES = 20_000
MAX_BYTES = 256 * 1024 * 1024
MAX_FILE_BYTES = 2 * 1024 * 1024
MAX_METADATA_BYTES = 64 * 1024 * 1024


def read_notice(root, relative):
    """Read trusted package-cache input; refuse redirects and nonportable names.

    This is not a hostile concurrent-filesystem sandbox. The collector compares
    the captured content with its inventory before including it in the archive.
    """
    parts = PurePosixPath(relative).parts
    if (not parts or PurePosixPath(relative).is_absolute()
            or "/".join(parts) != relative or any(p in {".", ".."} for p in parts)
            or any(c in relative for c in "\\:\0") or any(ord(c) < 32 for c in relative)):
        raise ValueError("notice has an unsafe relative path")
    path = root
    for part in parts:
        path = path / part
        if path.is_symlink() or (hasattr(path, "is_junction") and path.is_junction()):
            raise ValueError("notice path is redirected")
    if not path.is_file():
        raise ValueError("notice is not a regular file")
    with path.open("rb") as source:
        data = source.read(MAX_FILE_BYTES + 1)
    if len(data) > MAX_FILE_BYTES:
        raise ValueError("notice exceeds bundle byte limit")
    return data


def entry(archive, name, data):
    # Fixed metadata and stored bytes make output repeatable without depending
    # on a particular zlib version. Never rewrite the original notice text.
    info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
    info.create_system = 3
    info.external_attr = 0o100644 << 16
    info.compress_type = zipfile.ZIP_STORED
    archive.writestr(info, data)


def write_bundle(output, report, report_bytes, package_roots, normalized_digest):
    """Exclusively create an archive bound to exact report bytes and raw notices.

    package_roots is an ephemeral id->local-path mapping from the same Cargo
    metadata as report. No local paths enter the archive index. Ordinary failures
    remove the output created by this call; an existing destination is untouched.
    """
    if report.get("legal_approval") is not False or report.get("release_ready") is not False:
        raise ValueError("notice bundles require explicitly unapproved evidence")
    if len(report_bytes) > MAX_METADATA_BYTES:
        raise ValueError("notice report exceeds metadata byte budget")
    output = Path(output)
    index = {"format": 1, "scope": "observed-candidate-notices",
             "report_sha256": hashlib.sha256(report_bytes).hexdigest(),
             "legal_approval": False, "release_ready": False, "notices": [],
             "packages_without_observed_notices": []}
    total = 0
    seen = set()
    owned = False
    try:
        with output.open("xb") as destination:
            owned = True
            with zipfile.ZipFile(destination, "w", allowZip64=False) as archive:
                entry(archive, "cargo-evidence.json", report_bytes)
                for package in sorted(report["packages"], key=lambda p: p["id"]):
                    identity = package["id"]
                    if identity in seen or identity not in package_roots:
                        raise ValueError("duplicate or absent notice package identity")
                    seen.add(identity)
                    root = package_roots[identity]
                    prefix = hashlib.sha256(identity.encode()).hexdigest()
                    if not package["notice_files"]:
                        index["packages_without_observed_notices"].append(identity)
                    paths = set()
                    for notice in sorted(package["notice_files"], key=lambda n: n["path"]):
                        relative = notice["path"]
                        if relative in paths:
                            raise ValueError("duplicate notice path")
                        paths.add(relative)
                        if len(index["notices"]) >= MAX_FILES:
                            raise ValueError("notice bundle exceeds file budget")
                        data = read_notice(root, relative)
                        if normalized_digest(data) != notice["sha256_lf"]:
                            raise ValueError("notice changed after inventory")
                        total += len(data)
                        if total > MAX_BYTES:
                            raise ValueError("notice bundle exceeds aggregate byte budget")
                        name = f"notices/{prefix}/{relative}"
                        entry(archive, name, data)
                        index["notices"].append({"package_id": identity, "package_path": relative,
                                                "archive_path": name, "size": len(data),
                                                "sha256": hashlib.sha256(data).hexdigest(),
                                                "sha256_lf": notice["sha256_lf"]})
                index["notice_count"] = len(index["notices"])
                index["notice_bytes"] = total
                index_bytes = (json.dumps(index, sort_keys=True, indent=2) + "\n").encode()
                if len(index_bytes) > MAX_METADATA_BYTES:
                    raise ValueError("notice index exceeds metadata byte budget")
                entry(archive, "INDEX.json", index_bytes)
            destination.flush()
            os.fsync(destination.fileno())
    except BaseException:
        if owned:
            output.unlink(missing_ok=True)
        raise
    return {"notice_count": index["notice_count"], "notice_bytes": total,
            "packages_without_observed_notices": len(index["packages_without_observed_notices"])}
