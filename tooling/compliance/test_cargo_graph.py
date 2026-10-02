"""Candidate inventory evidence remains distinct from a legal disposition."""
import copy
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location("candidate_graph", Path(__file__).with_name("cargo_graph.py"))
graph = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(graph)


class GraphTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.packages = []
        self.nodes = []
        self.lock = []
        for name, license in [("mise","MIT"),("runtime","MIT OR Apache-2.0"),("builder",None),("dev-only","GPL-3.0-only")]:
            directory = self.root / name
            directory.mkdir()
            (directory/"Cargo.toml").write_text("",encoding="utf-8")
            (directory/"LICENSE").write_text("Synthetic notice for " + name,encoding="utf-8")
            self.packages.append({"id":name,"name":name,"version":"1.0.0","source":None,
                "manifest_path":str(directory/"Cargo.toml"),"license":license,"license_file":None})
            self.nodes.append({"id":name,"deps":[],"features":[]})
            self.lock.append(f'[[package]]\nname="{name}"\nversion="1.0.0"\n')
        self.nodes[0]["deps"] = [
            {"pkg":"runtime","dep_kinds":[{"kind":None}]},
            {"pkg":"builder","dep_kinds":[{"kind":"build"}]},
            {"pkg":"dev-only","dep_kinds":[{"kind":"dev"}]},
        ]
        self.metadata = {"packages":self.packages,"resolve":{"root":"mise","nodes":self.nodes}}

    def report(self, metadata=None):
        return graph.inventory(metadata or self.metadata,self.root,"x86_64-unknown-linux-gnu","a"*40,
            "\n".join(self.lock).encode())

    def test_nondev_closure_preserves_declared_alternatives_and_unknowns(self):
        report = self.report()
        self.assertEqual(report["package_count"],3)
        self.assertEqual({p["name"] for p in report["packages"]},{"mise","runtime","builder"})
        self.assertIn("MIT OR Apache-2.0",report["declared_license_counts"])
        self.assertIn("UNDECLARED",report["declared_license_counts"])
        self.assertFalse(report["legal_approval"])
        self.assertFalse(report["release_ready"])
        self.assertTrue(all(p["disposition"] == "unreviewed" for p in report["packages"]))
        self.assertNotIn(str(self.root),str(report))

    def test_unreadable_notice_directory_cannot_produce_partial_report(self):
        def unreadable(root, *, followlinks, onerror):
            self.assertFalse(followlinks)
            onerror(PermissionError("synthetic private path"))
            return iter(())

        with mock.patch.object(graph.os, "walk", side_effect=unreadable):
            with self.assertRaisesRegex(ValueError, "notice directory could not be read") as failure:
                self.report()
        self.assertNotIn("synthetic private path", str(failure.exception))

    def test_build_dependency_closure_and_normal_plus_dev_edges_are_retained(self):
        self.nodes[2]["deps"]=[{"pkg":"dev-only","dep_kinds":[{"kind":None},{"kind":"dev"}]}]
        report=self.report()
        self.assertEqual(report["package_count"],4)
        builder=next(p for p in report["packages"] if p["name"] == "builder")
        self.assertEqual(builder["dependencies"][0]["kinds"],["normal"])

    def test_notice_changes_are_evidence_and_external_declarations_are_unresolved(self):
        before=self.report()
        (self.root/"runtime/LICENSE").write_text("Changed synthetic notice",encoding="utf-8")
        self.packages[1]["license_file"]="../../outside-license"
        after=self.report()
        runtime=lambda r:next(p for p in r["packages"] if p["name"] == "runtime")
        self.assertNotEqual(runtime(before)["notice_files"],runtime(after)["notice_files"])
        self.assertEqual(runtime(after)["declared_license_file_status"],"unresolved-or-outside-package")
        self.assertFalse(after["release_ready"])

    def test_duplicates_missing_lock_entries_and_sensitive_sources_fail_closed(self):
        duplicate=copy.deepcopy(self.metadata)
        duplicate["packages"].append(duplicate["packages"][0])
        with self.assertRaisesRegex(ValueError,"duplicate"):
            self.report(duplicate)
        self.packages[1]["source"]="git+https://token:secret@example.invalid/repository"
        with self.assertRaisesRegex(ValueError,"credentialed"):
            self.report()
        self.packages[1]["source"]=None
        self.lock=self.lock[:1]
        with self.assertRaisesRegex(ValueError,"absent from Cargo.lock"):
            self.report()

    def test_notice_scan_excludes_symlinked_external_file(self):
        external=self.root/"outside"
        external.write_text("outside",encoding="utf-8")
        link=self.root/"runtime/NOTICE"
        try:
            link.symlink_to(external)
        except OSError:
            self.skipTest("symlink creation unavailable on this host")
        runtime=next(p for p in self.report()["packages"] if p["name"] == "runtime")
        self.assertEqual(runtime["skipped_links"],1)
        self.assertNotIn("NOTICE",[n["path"] for n in runtime["notice_files"]])


if __name__ == "__main__":
    unittest.main()
