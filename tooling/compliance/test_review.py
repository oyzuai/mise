import unittest
from review import approved, sensitive


class ReviewTests(unittest.TestCase):
    def review(self, identity, state="APPROVED", commit="head", user="owner"):
        return {"id": identity, "state": state, "commit_id": commit, "user": {"login": user}}

    def test_current_approval_from_owner(self):
        self.assertTrue(approved([self.review(1)], "head", ["owner"]))

    def test_old_commit_or_other_user_cannot_approve(self):
        self.assertFalse(approved([self.review(1, commit="old")], "head", ["owner"]))
        self.assertFalse(approved([self.review(1, user="outsider")], "head", ["owner"]))

    def test_dismissal_and_changes_requested_override_approval(self):
        for state in ["DISMISSED", "CHANGES_REQUESTED"]:
            self.assertFalse(approved([self.review(1), self.review(2, state)], "head", ["owner"]))

    def test_comment_does_not_manufacture_approval(self):
        self.assertFalse(approved([self.review(1, "COMMENTED")], "head", ["owner"]))

    def test_controls_and_native_manifests_need_review(self):
        for path in ["compliance/policy.json", ".github/workflows/build.yml", "x/AGENTS.md", "x/LICENSE-MIT", "go.mod"]:
            self.assertTrue(sensitive(path), path)
        self.assertFalse(sensitive("src/first_party.rs"))


if __name__ == "__main__":
    unittest.main()
