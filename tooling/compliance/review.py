"""Require an actual current-head GitHub approval for compliance-sensitive PRs.

Bootstrap is explicitly reported for the first foundation PR. After merging,
approvers come from the BASE policy, never a policy supplied by the PR itself.
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import urllib.request

from check import kind


def sensitive(path):
    return bool(kind(path) or path.startswith(("compliance/", "tooling/compliance/", ".github/workflows/"))
                or Path(path).name in {"AGENTS.md", "CLAUDE.md", "CODEOWNERS"})


def approved(reviews, head, owners):
    latest = {}
    for review in sorted(reviews, key=lambda item: item["id"]):
        if review["state"] in {"APPROVED", "CHANGES_REQUESTED", "DISMISSED"}:
            latest[review["user"]["login"].lower()] = review
    return any(review["state"] == "APPROVED" and review["commit_id"] == head
               for login, review in latest.items() if login in {x.lower() for x in owners})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    args = parser.parse_args()
    event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text())
    pr = event.get("pull_request")
    if not pr:
        print("No PR review to validate. Passing inventory checks is not distribution approval.")
        return
    base, head = pr["base"]["sha"], pr["head"]["sha"]
    if not all(re.fullmatch(r"[0-9a-f]{40}", sha) for sha in (base, head)):
        raise ValueError("invalid commit identity")
    command = ["git", "-C", str(args.root)]
    existing = subprocess.run(command + ["show", f"{base}:compliance/policy.json"], capture_output=True)
    if existing.returncode:
        print("BOOTSTRAP: base has no compliance policy. Human review of this foundation is required before merge.")
        return
    owners = json.loads(existing.stdout)["technical_reviewers"]
    changed = subprocess.check_output(command + ["diff", "--name-only", "--no-renames", base, head]).decode().splitlines()
    paths = [path for path in changed if sensitive(path)]
    if not paths:
        print("No compliance-sensitive tracked paths changed; copied-code review is still required by policy.")
        return
    repository = os.environ["GITHUB_REPOSITORY"]
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        raise ValueError("invalid repository identity")
    reviews = []
    for page in range(1, 21):
        request = urllib.request.Request(
            f"https://api.github.com/repos/{repository}/pulls/{int(pr['number'])}/reviews?per_page=100&page={page}",
            headers={"Authorization": "Bearer " + os.environ["GH_TOKEN"],
                     "Accept": "application/vnd.github+json", "X-GitHub-Api-Version": "2022-11-28"})
        with urllib.request.urlopen(request, timeout=15) as response:
            batch = json.load(response)
        reviews.extend(batch)
        if len(batch) < 100:
            break
    else:
        raise ValueError("review pagination bound exceeded")
    if not approved(reviews, head, owners):
        print("Human current-head approval required from: " + ", ".join(owners))
        print("Sensitive paths:\n" + "\n".join(paths))
        raise SystemExit(1)
    print("Current-head technical approval verified; legal/release approval remains separate.")


if __name__ == "__main__":
    main()
