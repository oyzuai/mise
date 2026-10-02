"""Notice review archives preserve raw evidence without approving obligations."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock
import zipfile

SPEC = importlib.util.spec_from_file_location("candidate_notices", Path(__file__).with_name("cargo_notices.py"))
bundle = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(bundle)


def normalized(data):
    return hashlib.sha256(data.replace(b"\r\n", b"\n")).hexdigest()


class BundleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.package = self.root / "package"
        self.package.mkdir()
        self.raw = b"Original synthetic copyright\r\nSynthetic license\r\n"
        (self.package / "LICENSE").write_bytes(self.raw)
        self.identity = "registry+https://example.invalid/index#fixture@1.0.0"
        self.report = {"legal_approval": False, "release_ready": False, "packages": [
            {"id": self.identity, "notice_files": [{"path": "LICENSE", "sha256_lf": normalized(self.raw)}]},
            {"id": "workspace:empty#empty@1.0.0", "notice_files": []}]}
        self.roots = {self.identity: self.package, "workspace:empty#empty@1.0.0": self.package}

    def write(self, name="bundle.zip"):
        output = self.root / name
        raw_report = (json.dumps(self.report, sort_keys=True) + "\n").encode()
        result = bundle.write_bundle(output, self.report, raw_report, self.roots, normalized)
        return output, raw_report, result

    def test_deterministic_archive_preserves_exact_bytes_hashes_and_unresolved_evidence(self):
        first, raw_report, summary = self.write()
        second, _, _ = self.write("second.zip")
        self.assertEqual(first.read_bytes(), second.read_bytes())
        with zipfile.ZipFile(first) as archive:
            index = json.loads(archive.read("INDEX.json"))
            self.assertEqual(archive.read("cargo-evidence.json"), raw_report)
            self.assertEqual(index["report_sha256"], hashlib.sha256(raw_report).hexdigest())
            record = index["notices"][0]
            self.assertEqual(archive.read(record["archive_path"]), self.raw)
            self.assertEqual(record["sha256"], hashlib.sha256(self.raw).hexdigest())
            self.assertEqual(record["sha256_lf"], normalized(self.raw))
            self.assertEqual(record["size"], len(self.raw))
            self.assertFalse(index["legal_approval"])
            self.assertFalse(index["release_ready"])
            self.assertEqual(index["packages_without_observed_notices"], ["workspace:empty#empty@1.0.0"])
            self.assertNotIn(str(self.root), json.dumps(index))
            self.assertEqual(len(archive.namelist()), 3)
        self.assertEqual(summary["notice_count"], 1)

    def test_changed_notice_and_budget_failures_remove_only_owned_partial_archive(self):
        (self.package / "LICENSE").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "changed after inventory"):
            self.write()
        self.assertFalse((self.root / "bundle.zip").exists())
        (self.package / "LICENSE").write_bytes(self.raw)
        for variable in ["MAX_FILES", "MAX_BYTES", "MAX_FILE_BYTES", "MAX_METADATA_BYTES"]:
            with mock.patch.object(bundle, variable, 0):
                with self.assertRaises(ValueError):
                    self.write()
                self.assertFalse((self.root / "bundle.zip").exists())
        (self.root / "bundle.zip").write_bytes(b"retain existing")
        with self.assertRaises(FileExistsError):
            self.write()
        self.assertEqual((self.root / "bundle.zip").read_bytes(), b"retain existing")

    def test_unsafe_duplicate_missing_and_approved_inputs_fail(self):
        notice = self.report["packages"][0]["notice_files"][0]
        for path in ["../LICENSE", "/LICENSE", "a//LICENSE", "a\\LICENSE", "C:LICENSE", "./LICENSE"]:
            notice["path"] = path
            with self.assertRaisesRegex(ValueError, "unsafe relative"):
                self.write()
        notice["path"] = "LICENSE"
        self.report["packages"][0]["notice_files"].append(notice.copy())
        with self.assertRaisesRegex(ValueError, "duplicate notice"):
            self.write()
        self.report["packages"][0]["notice_files"].pop()
        self.roots.pop(self.identity)
        with self.assertRaisesRegex(ValueError, "absent notice package"):
            self.write()
        self.report["legal_approval"] = True
        with self.assertRaisesRegex(ValueError, "unapproved"):
            self.write()
        self.assertFalse((self.root / "bundle.zip").exists())

    def test_redirected_notice_is_never_archived(self):
        source = self.package / "LICENSE"
        source.unlink()
        outside = self.root / "outside"
        outside.write_bytes(self.raw)
        try:
            source.symlink_to(outside)
        except OSError:
            self.skipTest("symlink creation unavailable on this host")
        with self.assertRaisesRegex(ValueError, "redirected"):
            self.write()
        self.assertFalse((self.root / "bundle.zip").exists())


if __name__ == "__main__":
    unittest.main()
