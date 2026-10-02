# Third-party compliance foundation

Status: operational guardrails proposed for maintainer review. The license policy
and every dependency disposition remain **unapproved**. Inventory consistency is
not permission to distribute and is not a legal opinion. The initial technical
review owner is **@micahlmartin**, selected by the maintainer. On 2026-10-02 the
maintainer also assigned him ownership of licensing and distribution decisions.
Ownership does not constitute approval of a dependency graph, policy or release;
the actual decision must still be recorded against the reviewed revision.
No first-party license is selected by these files.

## Rules for people and AI agents

Before importing/copying third-party code, changing a dependency, updating an
upstream pin or distributing an artifact, identify the affected component,
source/version, license text, notices and applicable obligations. Do not assume
the enclosing repository's MIT license covers dependencies, plugins, registry
data, assets, native libraries or downloaded tools. Generated code can still
contain third-party material and requires the same provenance review.

Preserve original copyright/license/NOTICE files and covered file headers. Mark
modifications when required by the applicable license, and record our changes
without relabeling upstream authorship. Moving code between files does not remove
its notices or obligations. Document license-expression choices and distinguish
alternative from cumulative terms; AI must not choose an unapproved legal policy.

Agents may collect evidence and propose a disposition. They must not approve
their own exceptions, claim a human reviewed an artifact, fabricate an approval
record, dismiss required reviews, or weaken a failing control. A baseline update
records observations only. AI must not submit a GitHub approval on behalf of the
human reviewer or bypass protections. Unresolved source/notice obligations block
the affected import or distribution, rather than causing an automatic exception.

These rules apply in all subdirectories; nested agent instructions cannot relax
them. A PR must state third-party impact (including "none" with a reason), identify
changed component/version/license/source records, explain notice/source handling
and provide actual validation. Preserve confidential information and credentials.

## What the guard checks

`tooling/compliance/check.py` uses only Python 3.11+ and Git. It hashes tracked
license/notice files and known dependency inputs across the tree, with LF newline
normalization for Windows/Linux consistency. It records Cargo lock entries and
npm lock v2/v3 package entries as **unreviewed**. Other manager inputs are tracked
for drift but do not receive a fabricated complete dependency graph. It never
downloads, builds or runs dependency code. Unknown license values remain unknown.

`baseline.json` is a factual snapshot. `components.json` records explicit copied
source components and exact notice hashes; dependency rows remain in the baseline.
Empty source-component records do not assert that the dependency audit is done.
`policy.json` deliberately has an empty approved-license list, no exceptions and
distribution blocked. The current checker rejects purported component approvals
and nonprovisional policy states. A future approved policy needs a reviewed checker
and release-audit implementation, not an agent flipping a JSON Boolean.

Run from the repository root:

```sh
python -m unittest discover -s tooling/compliance -p 'test_*.py'
python tooling/compliance/check.py
python tooling/compliance/check.py --base origin/main --report compliance-report.json
```

After investigating a deliberate dependency/notice change, stage the new input
files so Git's tracked-file list includes them, then regenerate the factual record:

```sh
python tooling/compliance/check.py --write-baseline
```

Review the resulting diff with the dependency change. Do not regenerate just to
silence an unexplained failure. Existing unreviewed rows are not grandfathered
as legally approved. The release-mode command **must fail with exit 2** while the
audit is incomplete:

```sh
python tooling/compliance/check.py --release
```

Its expected failure is regression-tested; a normal inventory job passing does not
mean a release gate passed. This foundation does not yet inspect compiled archives
or publish required source. Packaging jobs must not interpret this inventory job
as distribution clearance. Development CI compilation/test artifacts remain
unaudited engineering outputs, not compliance-certified product releases.

## Review and GitHub enforcement

### Candidate Cargo evidence

`tooling/compliance/cargo_graph.py` collects review evidence for the embedding
candidate with default features disabled and `rustls,vfox/vendored-lua` enabled:

```sh
python tooling/compliance/cargo_graph.py --target x86_64-unknown-linux-gnu --output cargo-evidence.json --notice-bundle cargo-notices.zip
```

Python 3.11+, Git, the candidate's Rust compiler and a previously provisioned Cargo
cache are required. The command runs only offline, locked Cargo metadata; it does
not compile, execute a backend, fetch missing packages or change the lockfile.
The other supported target filters are `aarch64-apple-darwin` and
`x86_64-pc-windows-msvc`. A target filter is not native execution evidence.
The embedding workflow collects one report per target after the native library
checks, explicitly provisioning the same locked, target-filtered Cargo metadata
query and feature set before
offline collection under Rust 1.95.0. Provisioning can use the network; collection
cannot. It retains reports
as `cargo-evidence-<target>-<commit>` artifacts for 30 days, including the report
and original observed notice bytes. Download and preserve the reviewed evidence
outside this expiring CI storage before any release review.
Collection failure fails that job; a successful upload is not legal approval or
proof of complete artifact obligations. Changes to the collector rerun this
matrix as well as the compliance guard.
Missing cached metadata is an error: provision it separately and repeat the
command. The report path must have an existing parent and is overwritten.
The optional `--notice-bundle` path must be different, have an existing parent
and not already exist; it is created exclusively and never replaces another file.

The ZIP contains the exact UTF-8/LF `cargo-evidence.json`, original notice bytes
under package-identity-hash directories and `INDEX.json`. The index binds the
report's raw SHA-256, each notice's package identity/original relative path,
raw SHA-256, LF-normalized SHA-256 and byte size. It explicitly lists packages
with no observed notice. Original line endings and copyright text are untouched.
Fixed archive metadata and uncompressed entries make repeated output identical
for identical report/notice bytes. A different host's metadata or working state
can legitimately change its report and therefore its archive.

Bundle collection rechecks notice hashes against the report and rejects changed,
redirected, nonregular or unsafe-path inputs. It limits notices to 20,000 files,
2 MiB each and 256 MiB total, plus 64 MiB each for report and index. Ordinary
failure removes this operation's partial archive; an existing destination stays
untouched. The report is written only after successful requested bundling. This
operates on a trusted provisioned package cache, not a hostile-filesystem sandbox.
The notice-bundle collector hash is also recorded in the report.

This archive is review evidence, not a release notice bundle or SBOM. It does not
select license alternatives, approve obligations, recover absent texts, include
installed tools' licenses or fulfill required source distribution. Both report
and index remain explicitly unapproved. Omitting `--notice-bundle` retains the
report-only operation.

The report follows normal and build dependency edges from mise, excludes edges
that are exclusively development dependencies, retains Cargo's declared license
expressions without selecting alternatives, and records lock checksums, resolved
features and LF-normalized hashes of conventional notice files and declared
license files. It binds the source revision, lockfile, metadata and collector
hashes, and reports tracked worktree changes. Untracked source and generated
inputs require separate review; a dirty report does not identify a clean release.
Cargo metadata can unify development features, so this is a candidate review
graph, not the exact compiled or distributed graph or a release SBOM.

Scans are bounded to 4,096 packages, 100,000 directory entries per package,
4,096 conventional notice files per package and 2 MiB per notice. Directory
enumeration failures reject collection rather than silently omitting notices.
No successful partial report is returned for an unreadable directory. Directory
symlinks/junctions and symlinked notices are excluded and counted; unresolved or
external declared license files are explicitly recorded. Repository package scans
may include nested package notices. Source headers, generated and vendored code,
native libraries, artifact contents and source-delivery obligations still need
review. Source identities containing URL credentials or queries are rejected;
absolute cache/workspace paths are not emitted. Every disposition remains
`unreviewed`, with `legal_approval: false` and `release_ready: false`.

The first Linux candidate collection at `3552b5d03111f6d3143233dbebb94a8368110a9e`
observed 955 packages. It was collected from a worktree reported as modified and
is exploratory evidence, not approval of that revision. Regression tests cover
normal/build/development edges, unknown declarations, notice drift, malformed
identities, missing lock entries and excluded external links.

### Approval enforcement

The workflow runs on every PR (no path filter), main pushes, review events and
manual dispatch. It checks current input hashes and component notices and retains
a report explicitly marked `legal_approval: false`, `release_ready: false`.

For subsequent PRs touching dependencies/notices/agent rules/workflows/compliance
files, `review.py` requires an actual APPROVED GitHub review for the current head
commit by a technical reviewer from the **base branch policy**. Stale, dismissed,
comment-only or other-user reviews cannot satisfy it. It requests read-only
permissions, executes no third-party code and uses no `pull_request_target` event.
The initial bootstrap PR has no base policy; it is visibly reported as bootstrap
and must receive maintainer review before merge, not silently claimed approved.

CODEOWNERS identifies sensitive areas and routes review. Repository administrators
must require the `License inventory and review` check, prohibit force pushes and
require code-owner review on the protected default branch. Protect the workflow,
checker and policy themselves; without server-side protection a contributor can
rewrite their own CI. Review count alone is not legal approval. AI instructions
cannot stop an administrator deliberately bypassing controls or prove that a
GitHub credential was operated by a human.

GitHub does not allow PR authors to approve their own PRs. With only one named
reviewer, PRs authored by that reviewer need another authorized human added by
reviewed policy change before strict review can be satisfied. Do not add a bot as
a workaround. Record live protection state in the setup/PR report; files in a
branch do not activate default-branch enforcement before they are merged.

## Required before distribution can be approved

1. Inventory exact shipped target/feature configurations, native/static libraries,
   assets, embedded registries and generated code. Separate development-only inputs
   from code/data actually redistributed. Reconcile binaries with the build graph.
2. Verify original license texts and provenance for every shipped component;
   record selected alternatives, obligations, reviewer and immutable evidence.
   Review unresolved/ambiguous/restrictive terms with qualified counsel.
3. Decide first-party licensing/contribution terms through human approval.
4. Produce version-matched full license texts, copyright/NOTICE attribution and
   any required covered source, including modifications, with durable availability
   instructions for recipients. A package inventory/SBOM is not a substitute.
5. Validate every release format actually includes its notices and required source
   references; keep the evidence with its artifact hashes. A repository-only file
   does not prove a standalone executable/installer carries the required material.
6. Audit downloaded/redistributed tools and plugins separately from mise's own
   source. Record the distribution and delivery model; do not infer rights from
   an installer's root license.
7. Approve policy and exceptions with named human authority; implement and connect
   a real artifact/source-availability gate before enabling release automation.

## Limitations

The scanner cannot identify arbitrary pasted code or all dependency declarations.
Manifest extensions, source-copy changes and unsupported ecosystems need human
review and deliberate inventory expansion. The baseline includes fixtures and
nonshipping inputs and is not an SBOM. Existing source is not retroactively legally
certified. No license allowlist, MPL source-availability fulfillment, legal signoff
or universal prevention of noncompliant release is claimed by these changes.

## Fork-specific automation status

All 35 inherited upstream workflows are preserved byte-for-byte in
`.github/upstream-workflows/`, outside GitHub's active workflow directory. This
includes publishing and upstream-specific maintenance/CI. Only the Oyzu compliance
workflow is introduced as active automation. Restoring a workflow is a reviewed
change: audit its permissions, destinations, secrets and artifact/license gates.
Do not enable upstream publication jobs just to test library integration.
The preserved root and nested license texts are fingerprinted in components.json.
The fork baseline is captured from its actual source revision, which is distinct
from the earlier experiment's pinned revision.
