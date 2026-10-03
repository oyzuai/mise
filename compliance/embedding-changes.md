# Oyzu embedding changes

Experimental implementation of [Oyzu OEP-0003](https://github.com/micahlmartin/oyzu/tree/codex/mise-qualification/docs/proposals/OEP-0003-mise-integration).
This document records modifications, not licensing approval or production readiness.

Upstream base: `b1b8d3e4aed6a0a610fdd1d845df11da470afd08` in `jdx/mise`.
Governance foundation: `503e384d1` on `oyzuai/mise`'s
`codex/license-compliance` branch. Upstream MIT license and notices are retained;
these changes do not select licenses for dependencies or downloaded tools.
The manifest exposes the existing vendored Lua feature for external consumers; no dependency version or lockfile changes are introduced by that forwarding feature.

| Patch | Purpose | Removal condition |
| --- | --- | --- |
| Process initialization | Explicit private roots, empty configuration and a single-use context; reject inherited overrides | Upstream offers an equivalent no-discovery embedding context |
| HTTP transport hook | Route the shared HTTP client through an explicit callback; no callback denies requests | Upstream offers an equivalent mandatory transport hook |
| Frontend ownership | Use the caller's image identity and reject mise shim publication | Upstream exposes equivalent frontend control |
| Verbatim PATH | Preserve duplicate entries when composing live shell state | Upstream offers equivalent lossless composition |
| Node archive facts | Reuse the Node backend's artifact/mirror selection and native path helpers for explicit targets without installing or executing them | Upstream exposes an equivalent data-only target layout boundary |
| Go archive facts | Share Go artifact/mirror selection with the backend's lock URL path and its archive-root extraction constant; expose fresh-archive layout facts without Git discovery or execution | Upstream exposes an equivalent data-only target layout boundary |

The library-only `oyzu-embedding-check` example exercises initialization in fresh
child processes with invalid ambient project/global configuration, absent and
supplied transports, rejected overrides/backend identifiers, settings resets and
duplicate PATH entries. A fifth scenario uses mise's actual Node catalog parser
and prefix resolver with fixture metadata supplied by the transport callback.
It initializes the backend registry explicitly and resolves `22` to `22.15.0`.
The public mise aggregation service is disabled in embedding mode; publisher
metadata is routed through the supplied transport. Run with Rust 1.95:

```sh
cargo run --locked --no-default-features --features rustls,vendored-lua --example oyzu-embedding-check
```

This example does not build or invoke the mise CLI. The transport hook is not a
network sandbox: other clients and subprocesses still require Oyzu's isolated
worker and acquisition broker. Only four core identifiers are currently admitted;
actual tool installation/execution, Aqua/npm admission, native platform qualification,
secure installation, receipts and Oyzu lock publication remain separate work.
The callback response and artifact bytes must be checked by the frontend.

@micahlmartin owns patch review, upstream refreshes and licensing decisions.
Every refresh must rebase/reapply these patches, rerun their conformance checks
and the Oyzu qualification matrix, review changed dependencies and notices, and
produce a separately reviewed exact revision update in Oyzu. Neither passing
tests nor this inventory authorizes distribution.

On Linux, all five scenarios and strict Clippy for the library/conformance target
passed with Rust 1.95. The `Oyzu embedding boundary` workflow runs those checks on
native Linux, Windows and macOS hosts. A declared matrix is not passing evidence;
review the run for the exact candidate head. Full upstream CLI checks are excluded
because Oyzu does not build or invoke a separate mise executable.

Native macOS run 36967923522 identified `__CF_USER_TEXT_ENCODING` in a child
created with `env_clear`. The embedding allowlist permits that variable only on
macOS; inherited mise overrides still fail. The corrected native run must pass
before macOS qualification is claimed.

`Session::node_archive_facts` accepts an exact stable version and one of the
initial Linux amd64/gnu, Darwin arm64/native or Windows amd64/msvc targets. It
returns archive/checksum/signature locations, archive kind, strip prefix and
upstream Node/npm launcher/PATH locations. Upstream lock metadata and this seam
share `NodePlugin::binary_artifact`; native Node/npm launches share the same
relative-path helpers. Target facts do not fetch, execute, assert artifact
availability or verify publisher evidence. The caller must convert them into an
admitted Oyzu layout only after obtaining exact bytes/size and verification.
In particular, the observed `npm.cmd` path is not a product shim or typed launcher.

The conformance example now has six process scenarios. It rejects planning when
Node is not admitted; for two versions and three targets it checks exact facts,
upstream URL parity and zero extra transport calls. These fixture checks are not
native archive-layout parity or real backend installation qualification. No
dependency or license/notice file changes accompany this seam.

`Session::go_archive_facts` uses the same bounded exact-stable-version and initial
target validation as Node, while enforcing Go's own immutable session admission.
The Go backend owns the fact type and `binary_artifact` calculation used by both
its existing `get_tarball_url` and this interface. The `go` strip prefix is shared
with upstream extraction. Results contain the archive and checksum-sidecar URLs,
format, strip prefix, executable path, bin path and fresh-layout GOROOT path.
Legacy nested installations and mutable GOPATH/package installation are not
projected. The shared validator accepts full stable SemVer, so legacy archive
versions written without a patch component are not exposed by this interface.

A seventh isolated conformance scenario checks two Go versions across three
targets, exact values, parity with the upstream artifact URL method, rejected
versions/targets, separate Node/Go admission and zero transport callbacks. It
does not use Go's Git discovery, download archives or run Go. Catalog membership,
checksum authenticity, native archive layout/executable parity and broker route
mapping remain qualification gates. No dependency or upstream notice changed.

An eighth isolated scenario exercises the existing Java backend's target-aware
`resolve_lock_info` API against supplied synthetic Temurin metadata for Linux,
Darwin ARM64 and Windows. It checks URL/checksum preservation, target-specific
archive filtering, unavailable-version denial, isolated Java admission and one
metadata callback per target. It neither downloads a JDK nor claims a Java layout
plan, publisher verification or install parity. Java production source is unchanged;
this adds evidence for reusing its parser without creating another catalog.

`Session::tool_aliases` projects the source revision's baked registry onto the
session's admitted core backends. It returns short names, registry aliases and
canonical core IDs; it rejects canonical rebinding, ambiguous aliases and drift
that removes the admitted backend. It does not read floating registry caches,
ambient aliases or project configuration. Embedded settings also disable the
floating registry, including after settings reload. A ninth isolated scenario
checks all four initial core tools, unadmitted-tool exclusion and reload stability;
the existing empty-admission scenario checks that no aliases are exposed.
This is name projection, not version resolution or backend qualification. No
dependency, registry data, copyright or license notice changes accompany it.

`Session::resolve_node_version` adds bounded metadata-only selection for admitted
Node. Its adapter lives under the Node backend and composes the existing remote
catalog, aliases, ordering, fuzzy prefix matcher and npm semver range filter.
It intersects the request with every supplied native constraint, then chooses
the latest matching canonical stable catalog entry. Exact pins must exist in
the catalog; no installed, path, system, ref or symbolic fallback is accepted.
Inputs are validated before metadata access. Transport and operation-private
catalog caches can be used, but no archive is acquired and no tool is installed
or executed. Target availability, publisher verification, release-age policy,
lock authoring and the other backends' selection remain separate responsibilities.
Two fresh-process scenarios exercise supplied-catalog selection and no-transport
denial; existing empty admission also rejects this operation. Original notices
are unchanged and no dependency or registry data is added.

`Session::node_archive_metadata` binds the existing target archive facts to a
publisher-declared checksum entry. Node's existing checksum fetch path is shared
with the embedding adapter. The shared hash module now offers bounded checked
SHA-256 manifest decoding, reusing the legacy parser's field splitting while
rejecting duplicate names, malformed hashes, extra fields and oversized input.
Legacy parser tolerance remains unchanged. The returned checksum is explicitly
declared metadata, not signature verification, content verification or authority.
Missing target entries fail without source compilation or another target fallback.
A twelfth fresh-process scenario covers three targets, manifest cache reuse,
malformed/ambiguous/missing entries and parser bounds; existing no-transport and
admission scenarios cover the new operation. No dependency or notice changed.

`Session::go_archive_metadata` reuses Go's target archive facts and checksum URL,
fetches only its declared SHA-256 through the supplied HTTP transport, bounds the
text to 128 bytes and rejects anything except one 64-character hexadecimal digest
with optional surrounding whitespace. Output normalizes hex case. Session backend,
exact stable version and target admission happen before acquisition. The operation
does not establish catalog membership, authenticated publisher evidence, artifact
size/content or installation authority. No dependency or notice changed.
A thirteenth fresh-process scenario covers three targets, normalization, malformed
and oversized checksums and invalid pre-acquisition inputs; existing scenarios
also verify backend admission and supplied-transport denial.

`Session::resolve_go_version` reuses the shared admitted selector owned by
`src/plugins/core/embedding_selection.rs`; Node keeps its selection semantics.
Go's `go/embedding.rs` owns the bounded official Go release JSON adapter specified
by OEP-0003. Selection returns both version and exact UTF-8 catalog SHA-256.
The earlier candidate GitHub-tag implementation was removed to correct its
mismatch with that requirement; ordinary upstream Go/GitHub behavior is restored.

The adapter rejects malformed/duplicate release records, text over 16 MiB, more
than 100,000 releases and oversized version strings. It excludes unstable and
noncanonical stable versions, reuses the upstream Go comparator and requires exact
pin membership. Selectors are checked before acquisition; transport owns streaming
limits and deadlines. Seven scenarios cover selection/cache/digest, missing metadata,
duplicate/malformed/invalid-UTF-8 records and both catalog limits. Artifact-file parity, native
directive discovery, publisher verification and production wiring remain unfinished.
No dependency, registry data or preserved notice changed.

Go target metadata now requires a stable release and exact upstream-derived
filename in the same official catalog used for selection. `go/embedding.rs` owns
both decoding and target-record matching; Go's shared artifact URL/layout logic
remains in its parent backend. File OS/architecture/version/kind, positive byte
size and SHA-256 must agree with the target; the existing sidecar is then fetched
and must match the catalog hash. Output adds declared size and catalog digest.
No file record is treated as publisher authentication or acquired-byte proof.

Stable canonical releases allow at most 4,096 file records with nonempty bounded,
unique filenames. Missing/invalid target records fail before sidecar acquisition.
Conformance covers three targets, all identity fields, absent release/target,
malformed sidecars and hash disagreement. Two additional process scenarios reject
duplicate and excessive file records. No dependency or notice changed.

The library conformance executable optionally accepts an external captured fixture
through its parent-only `OYZU_GO_METADATA_FIXTURE` environment variable. The parent
bounds it to 32 MiB and copies it into a fresh worker state; the variable is not
inherited by the embedding child. `examples/oyzu-embedding-check/go_replay.rs`
checks captured response sizes/hashes and compares exact version, target, upstream
archive URL, declared size/hash and catalog digest for every expected case. Only
listed response URLs are served, with no transport fallback. This is test input,
not source approval. Ordinary conformance remains independent of network/fixtures.
A fixture provisioned by Oyzu's public metadata capture helper covered Go 1.24.13
and 1.25.0 on the initial three targets. Linux replay passed with Docker networking
disabled; changing an expected size failed. No archive or target executable ran.

Native embedding CI now provisions the optional real Go metadata replay on each
initial host. Its Python capture helper is fetched from Oyzu commit
`f35c01d5bc8348b4ee6efee8e52c5352e0003dfd` and must match SHA-256
`159f0213c465379b2100a4b1c2b074f05aad6db3a6ea221ddcc79e103b3e8c2c`
before execution. It is staged outside the fork checkout in a temporary repository
shape so its output-location guard remains effective. Captured fixtures and reports
are retained as target/revision artifacts for 30 days, including after a replay
failure when capture succeeded. Baseline conformance runs independently first.
Live publisher failures remain failures, never synthetic substitutions.
The example-file workflow filters were also corrected to separate the entrypoint
from its directory glob. Workflow syntax/path filters and the exact provisioner
step passed locally on Windows; new native workflow execution remains pending.
No dependency, preserved notice or product Rust implementation changed.

### Captured Java metadata replay

The library-only harness accepts OYZU_JAVA_METADATA_FIXTURE for a bounded 32 MiB
fixture produced by Oyzu's pinned capture_java_metadata.py. It verifies original
catalog sizes/hashes, admits only Java, and supplies the three captured catalog
URLs through the embedding callback. The actual Java backend resolves
`temurin-21.0.6+7.0.LTS` for Linux amd64, macOS arm64 and Windows amd64 and must
match independent expected archive URLs/checksums; an unavailable version fails.
Only three catalog requests are allowed. All ordinary conformance cases still
run. The environment variable selects test data only, never a product route.

Linux strict library/example Clippy, pinned formatting and 22 ordinary cases plus
this three-target replay pass. Replay runs with networking disabled. Native CI
now captures and retains the fixture and runs the same case on all three hosts;
those outcomes are pending. This changes first-party harness/CI/documentation
only, with no dependency or upstream notice changes. Declared checksum agreement
is not publisher verification, exact/range Java resolution admission, artifact
installation, legal approval or production integration. No mise CLI is built or
invoked; the example links the existing library candidate.

### Bounded embedded catalog collection

The shared HTTP client now offers get_bytes_bounded through the existing
embedding/authorization transport path. It checks declared/body lengths and
accumulates decoded chunks only within the caller's limit; response loss returns
an error rather than partial metadata. Go's embedded catalog uses 16 MiB and its
checksum sidecar 128 bytes. Java's embedded catalog uses 16 MiB and rejects more
than 100,000 decoded records; ordinary nonembedded Java retains its existing path.
Java's record count is checked after bounded-byte JSON decoding, so this is not
a complete parser-allocation or process-memory limit. Provider buffering before
returning a response, transport deadlines and executor memory limits remain
separate obligations. No dependency or original notice changes are introduced.

Bounded-collection validation: two utility tests cover exact limits, declared
length rejection, unknown-length chunk accumulation and interrupted streams.
Strict utility and library/example Clippy, Rust 1.95 formatting and all 22 ordinary
boundary cases plus real Go (six cases) and Java (three targets) replay passed on
Linux. Combined replay ran with container networking disabled. CI runs the new
utility tests on all three hosts. Java capture/replay now runs after successful
shared boundary checks even if independent Go capture fails; failure still fails
the job and is never replaced by synthetic inputs. Run 37004801980 passed Java
replay on Windows/macOS, while Ubuntu's Go capture returned HTTP 404 before Java.
A fresh local seven-response Go capture succeeded; that does not erase the CI
failure or prove its root cause. New bounded-reader native CI remains pending.

### Targeted Java version selection

Session::resolve_java_version(request, constraints, target) selects a cataloged
GA Temurin HotSpot JDK for Linux amd64 GNU, macOS arm64 or Windows amd64 MSVC.
It reuses Java's existing catalog ordering and fuzzy prefix matcher. Numeric
prefixes (21), vendor prefixes (temurin-21), exact catalog versions and latest
are accepted; all additional constraints must match the same version. The result
retains the complete vendor/build identity, such as temurin-21.0.6+7.0.LTS.
No SemVer conversion or stripping of build metadata occurs. The shared ordering
now saturates feature preference at zero for records with more than ten features,
avoiding arithmetic underflow without changing ordinary catalog ordering.

Selectors are at most 128 ASCII alphanumeric/period/plus/hyphen bytes; at most
256 additional constraints are allowed. Unsupported request modes, other vendors,
range operators, unknown targets and malformed/oversized input fail before any
metadata acquisition. A missing version or empty intersection fails after catalog
lookup. There is no installed-version or ambient-config fallback. Java listing
and embedding selection share the same ordering implementation.

This is an experimental selection seam, not production backend admission. Native
builder range-language translation, explicit prereleases, catalog ambiguity and
publisher verification, metadata provenance returned to the caller, the complete
target matrix and production broker/worker integration remain open. The frontend
still owns policy, request identity, Oyzu TOML/locks and installation. No new
license, dependency or notice is introduced.

Validation of Java selection: Linux strict library/example Clippy, Rust 1.95
formatting and library-only build pass. With container networking disabled, all
22 ordinary boundary scenarios plus captured Go and Java replay pass. Java now
checks exact/prefix/latest intersection and unchanged full identity on all three
targets, conflicts, missing versions, unsupported selectors/targets and excessive
constraints. Invalid input makes no transport calls. Native CI for this new seam
is pending. Compliance inventory is unchanged; 24 tests pass with two Windows
symlink skips. This does not qualify the remaining Java admission obligations.

### Python precompiled catalog bounds (qualification in progress)

Both precompiled catalog fetch paths now reuse the bounded HTTP reader with a
16 MiB body cap. The application-level gzip decoder reads at most 16 MiB plus
one overflow-probe byte and rejects overflow before version selection. Invalid
UTF-8, truncated gzip and checksum errors fail the fetch. Existing platform,
flavor, ordering and locked-artifact selection logic is unchanged. This bounds
catalog input, not total process memory or archive installation size.

The Linux library regression passes exact-limit acceptance, overflow, compressible
expansion, invalid UTF-8, truncation and checksum corruption. Scoped strict library
and embedding-example Clippy and Python source formatting pass. The three-host
embedding workflow now runs this regression and includes Python in formatting
checks; native CI results are still pending. This is not Python backend admission
or release approval. No dependency, source pin or license/notice file changed.

On 2026-10-02, bounded read-only observations of the upstream catalogs for
x86_64 Linux GNU, aarch64 macOS and x86_64 Windows MSVC measured respectively
9,402/195,798, 9,146/181,436 and 7,913/158,412 compressed/decoded bytes. These
observations used the explicit Oyzu metadata qualification User-Agent and are
size compatibility evidence only, not retained catalog provenance or parser replay.

All 22 existing fresh-process embedding harness scenarios also pass on Linux
with this change. The run builds the library/example only, not a mise executable.

### Embedded Python locked-catalog selection (validation in progress)

The explicit-target Python precompiled catalog path now checks the embedding
context before accepting upstream's selection. When a locked filename is supplied,
an absent or substituted candidate is an error. Unlocked selection still uses
upstream ordering, and ordinary nonembedded refresh retains its existing fallback.
The check returns metadata only; it does not verify publisher identity, authorize
installation or replace attestation enforcement. Direct locked-URL installation
and complete Oyzu worker integration remain separate qualification requirements.

A library regression exercises exact old-build retention, unlocked newest-build
selection, missing/empty/wrong-version denial and unchanged ordinary fallback.
The native workflow includes this alongside the gzip boundary test. Both Python
regressions, scoped strict library/example Clippy, formatting and all 22 existing
embedding scenarios pass on Linux. The compliance inventory and 24 guard tests
pass (two Windows symlink skips). Native CI for this revision remains pending;
no dependency or preserved notice changes are part of this change.

### Captured Python catalog replay

An explicitly ignored library test, `python_catalog_captured_replay`, reads an
independently retained `OYZU_PYTHON_METADATA_FIXTURE`. It verifies the three target
URLs, compressed and decoded byte counts/hashes, then passes the original gzip
bytes through the Rust decoder and the decoded catalog through the embedded
selector. For Python 3.12.13 it requires the exact 20250323 install-only artifact
on every target, then removes that line and requires rejection of substitution.
The fixture is bounded to 68 MiB and each compressed/decoded catalog to 16 MiB.

Run it explicitly with `cargo test --locked --lib --no-default-features --features
rustls,vendored-lua python_catalog_captured_replay -- --ignored`, setting the
fixture environment variable first. The input is produced by the public Oyzu
`tooling/mise-upstream/capture_python_metadata.py` helper. No network or archive
acquisition occurs in this test. Ordinary test runs report it ignored; this is
wired into native CI with a revision/hash-pinned capture helper and retained
fixture/report artifacts. Linux replay passed with Docker networking disabled
against the retained 2026-10-02 capture. Rust 1.95 formatting and scoped strict
library/example Clippy pass; native replay CI remains pending.
This adds first-party verification to existing MIT-covered source; no upstream
notices, dependencies or license choices change. It does not exercise the HTTP
transport, public Session API, attestation verification or installation.

### Python catalog-only artifact lookup (validation in progress)

`Session::python_catalog_artifact` admits the initial three explicit targets and
an exact stable CPython version. It fetches only the bounded precompiled catalog
through the supplied transport, reuses upstream selection with locked-filename
retention, and returns the exact compressed catalog digest plus filename, release
and archive URL. Complete filename validation admits only the default install-only
and install-only-stripped tar.gz layouts with an eight-digit release identity.
Host settings do not override the explicit target. Invalid target/version/locked
filename inputs fail before transport. Missing or unsupported selections fail.

This intentionally does not call `resolve_lock_info`, whose provenance path can
download an artifact. No archive, checksum sidecar, subprocess, installation or
attestation operation runs here. Catalog integrity identifies the observed bytes;
it does not establish artifact integrity or publisher authenticity. Version
constraint resolution, snapshot reuse, checksum/attestation acquisition and Oyzu
worker wiring remain outstanding. The real-catalog replay now also checks the
returned locked facts and URL. Linux real-catalog replay, formatting and scoped strict library/example Clippy
pass. Three new fresh-process Session scenarios cover catalog-only transport,
offline denial and backend admission; all 25 ordinary harness scenarios pass
on Linux. Native CI includes these scenarios; results for this API remain pending. Original notices and
dependencies remain unchanged.

### Python declared checksum lookup (validation in progress)

`Session::python_archive_metadata` composes catalog-only selection with a bounded
release `SHA256SUMS` fetch through the supplied transport. It reuses the shared
strict SHA-256 parser: at most 8 MiB and 4,096 unique filenames, valid UTF-8,
well-formed digests, and an exact selected filename. Duplicate, missing or invalid
entries fail. The result retains catalog facts and adds the declared artifact
SHA-256, checksum URL and exact checksum-response digest. It does not download
or execute the artifact, verify attestations, or authenticate the publisher.

Three additional public Session scenarios cover valid, duplicate and missing
checksum records on all initial targets and require only catalog/checksum requests.
Linux formatting, scoped strict library/example Clippy and all 28 ordinary
embedding harness scenarios pass. Native validation remains pending. A read-only observation of upstream
release 20260807 on 2026-10-02 measured 122,082 bytes and 852 checksum rows, below
the parser limits; this is size evidence, not retained provenance or real checksum
replay. Dependencies, upstream notices and licensing decisions are unchanged.

### Python public API metadata replay (validation in progress)

The library-only harness now recognizes OYZU_PYTHON_METADATA_FIXTURE generated
with the capture helper's --with-checksums option. A fresh child admits only Python,
validates the three fixture targets and original response hashes, supplies captured
binary responses through transport, and checks the public archive metadata result
against the independent expected artifact/checksum identities. It requires exactly
six requests (catalog and checksum per target); any uncaptured request fails.
Fixture input is bounded to 100 MiB, each decoded response to 16 MiB, and production
catalog/checksum limits remain enforced by the API. Existing private decoder replay
remains separate. Linux passes all 28 ordinary scenarios and real Python metadata replay with
container networking disabled. Formatting and strict library/example Clippy pass.
Native CI uses the pinned checksum-capable capture helper and runs both private
decoder and public Session replay; native results remain pending.

This adds first-party conformance code without changing dependencies or notices.
It does not authenticate publishers, acquire archives or authorize distribution.

## External consumer feature selection

The public `vendored-lua` Cargo feature forwards to the existing
`vfox/vendored-lua` feature. External consumers must use
`default-features = false, features = ["rustls", "vendored-lua"]` on their pinned
mise dependency. Cargo rejects a slash-containing feature name inside a dependency
feature list, so the earlier package-local invocation could not be copied into
Oyzu's manifest. No package version, upstream notice or license alternative is
changed. Existing default features remain unchanged. Qualification and candidate
graph collection now use the same public feature selection as a consumer.


## Rust native installation boundary

The embedding session now admits `rust` and provides `install_rust` for exact
stable compiler versions. It calls the existing Rust backend installer with the
minimal profile in explicit private Cargo/rustup homes, then returns the verified
private sysroot for frontend publication. No separate mise executable is invoked.
Bootstrap HTTP uses the supplied callback; rustup subprocess downloads use their
normal public paths. The caller must not advertise this as enforced proxy support.
The frontend owns lock identity, final storage and cached restoration. No new
third-party source, dependencies or license alternatives are introduced.

## Host-verified Rust distribution directory

`Session::install_rust_from_directory` extends the existing exact Rust installer
with an explicit absolute local distribution directory. It uses the native
backend's typed install environment for Rustup distribution/update roots; ambient
environment admission is unchanged. The frontend must verify the directory's
pinned inputs and isolate installer networking. The ordinary installer remains
available for existing standalone callers. This is first-party boundary code;
no dependencies, licenses or upstream notices change. Product-side installation
and offline replay are the focused validation; broad qualification is separate.

The frozen distribution method also disables rustup's toolchain-install self-update. The first isolated proof installed Rust but correctly failed when rustup requested unpinned rolling self-update metadata; the bootstrap remains owned by the caller's exact recipe.
