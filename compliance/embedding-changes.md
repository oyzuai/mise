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
