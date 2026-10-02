"""Regression checks for the compliance guard's failure boundaries."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("compliance_check", Path(__file__).with_name("check.py"))
guard = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(guard)


class GuardTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)

    def track(self, name, text):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
        subprocess.run(["git", "-C", str(self.root), "add", name], check=True, capture_output=True)
        return path

    def test_new_dependency_and_notice_changes_are_detected(self):
        self.track("LICENSE", "original notice\n")
        before = guard.snapshot(self.root)
        self.track("LICENSE", "changed notice\n")
        self.track("nested/Cargo.toml", '[package]\nname="new"\n')
        self.assertEqual(guard.drift(before, guard.snapshot(self.root)), ["LICENSE", "nested/Cargo.toml"])

    def test_deleted_notice_is_detected(self):
        self.track("vendor/COPYING", "keep me\n")
        before = guard.snapshot(self.root)
        subprocess.run(["git", "-C", str(self.root), "rm", "-f", "vendor/COPYING"], check=True, capture_output=True)
        self.assertEqual(guard.drift(before, guard.snapshot(self.root)), ["vendor/COPYING"])

    def test_cargo_records_never_claim_license_approval(self):
        self.track("Cargo.lock", 'version=3\n[[package]]\nname="third-party"\nversion="1.0.0"\nsource="registry+https://example.invalid"\n')
        package = guard.snapshot(self.root)["packages"][0]
        self.assertIsNone(package["license"])
        self.assertEqual(package["disposition"], "unreviewed")

    def test_non_cargo_inputs_are_tracked(self):
        for name in ["requirements-dev.txt", "go.sum", "build.gradle.kts", "pnpm-lock.yaml", "vendor/LICENSE-MIT"]:
            self.track(name, "test\n")
        self.assertEqual(len(guard.snapshot(self.root)["inputs"]), 5)

    def test_newlines_do_not_produce_platform_drift(self):
        self.assertEqual(guard.digest(b"notice\r\n"), guard.digest(b"notice\n"))

    def test_duplicate_json_is_rejected(self):
        path = self.track("bad.json", '{"schema":1,"schema":2}')
        with self.assertRaises(ValueError):
            guard.read_json(path)

    def test_forged_component_approval_is_rejected(self):
        with self.assertRaises(ValueError):
            guard.verify_components(self.root, {"schema": 1, "components": [{"id": "x", "review_status": "approved"}]})

    def test_notice_escape_is_rejected(self):
        with self.assertRaises(ValueError):
            guard.verify_components(self.root, {"schema": 1, "components": [{"id": "x", "review_status": "pending",
                "origin": "upstream", "scope": "test", "notices": [{"path": "../outside", "sha256_lf": "0" * 64}]}]})

    def test_release_check_cannot_be_green(self):
        directory = self.root / "compliance"
        directory.mkdir()
        policy = {"schema": 1, "status": "provisional", "distribution": "blocked_pending_audit", "approved_license_expressions": []}
        (directory / "policy.json").write_text(json.dumps(policy))
        (directory / "components.json").write_text('{"schema":1,"components":[]}')
        (directory / "baseline.json").write_text(json.dumps(guard.snapshot(self.root)))
        result = subprocess.run([sys.executable, str(Path(__file__).with_name("check.py")), "--root", str(self.root), "--release"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertIn("RELEASE BLOCKED", result.stderr)


if __name__ == "__main__":
    unittest.main()
