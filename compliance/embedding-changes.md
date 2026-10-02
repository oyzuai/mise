# Oyzu embedding changes

Experimental implementation of [Oyzu OEP-0003](https://github.com/micahlmartin/oyzu/tree/codex/mise-qualification/docs/proposals/OEP-0003-mise-integration).
This document records modifications, not licensing approval or production readiness.

Upstream base: `b1b8d3e4aed6a0a610fdd1d845df11da470afd08` in `jdx/mise`.
Governance foundation: `503e384d1` on `oyzuai/mise`'s
`codex/license-compliance` branch. Upstream MIT license and notices are retained;
these changes do not select licenses for dependencies or downloaded tools.
No dependency manifest or lockfile is changed by this embedding patch.

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
cargo run --locked --no-default-features --features rustls,vfox/vendored-lua --example oyzu-embedding-check
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
