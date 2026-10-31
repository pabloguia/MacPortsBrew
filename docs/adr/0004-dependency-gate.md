# ADR-0004 — The dependency gate: licences, deployment floor and build scripts

- **Status:** accepted
- **Date:** 2026-09-02
- **Pass:** 01-FOUNDATION-PASS
- **Supersedes:** the working assumption in `../spec/08-distribution-and-project.md` that the allow-list would hold only the eight named licences

## Context

`../spec/07-quality-and-testing.md` gives the dependency gate three axes:
licence (ADR-0001), deployment floor (ADR-0002), and the `unsafe` and
build-script surface a crate brings into a binary that performs privileged
operations. `08-distribution-and-project.md` says anything outside its
licence allow-list, and any crate with a non-trivial `build.rs`, needs a
decision recorded in an ADR before it is added. This is that record for the
foundation pass's dependency set, and the rule for extending it.

What the toolchain and the chosen crates bring in, read from
`cargo metadata` and from the crates' sources in the local registry on
2026-09-02 (rustc 1.98.0):

**Licences outside the eight named in `08`:** one. `unicode-ident`
(pulled in by `proc-macro2`, which every `serde` derive needs) is licensed
`(MIT OR Apache-2.0) AND Unicode-3.0`. Unicode-3.0 is the Unicode License v3,
a permissive attribution licence; it is compatible with Apache-2.0
redistribution. No copyleft licence of any kind is in the graph
(`scripts/check-licences.sh`, 60 packages, 2026-09-02).

**Crates with a `build.rs`, and what each script does** (read in full from
`~/.cargo/registry/src/`, 2026-09-02):

| Crate | Lines | What the script does |
|---|---|---|
| `proc-macro2` 1.0.107 | 267 | Runs `$RUSTC --version`, compiles a probe file to `OUT_DIR`, emits `rustc-cfg` flags for compiler features. |
| `quote` 1.0.47 | 32 | Runs `$RUSTC --version`, emits `rustc-cfg`. |
| `serde` 1.0.229 | 69 | Same probe pattern; writes only under `OUT_DIR`. |
| `serde_core` 1.0.229 | 113 | Same; reads `TARGET` to gate 128-bit integer support. |
| `serde_json` 1.0.151 | 30 | Reads `CARGO_CFG_TARGET_*`, emits `rustc-cfg`; spawns nothing. |
| `zmij` 1.0.23 | 45 | Float-formatting dependency of `serde_json`; `$RUSTC --version` probe, emits `rustc-cfg`. |
| `thiserror` 2.0.20 (dev only, via `ratatui`) | 195 | Same probe pattern. |
| `rustversion` 1.0.23 (dev only) | 119 | Same; writes one file under `OUT_DIR`. |
| `instability` 0.3.13 (dev only) | 3 | Emits one `rustc-check-cfg` line. |
| `libc` 0.2.191 (through `sha2` → `cpufeatures`, arm64; ADR-0007; to be admitted by P/01, staged and not yet applied on 2026-10-30) | 630 | Runs `$RUSTC --version` (through `RUSTC_WRAPPER` if set, like the others), reads `CARGO_CFG_*` and `RUST_LIBC_UNSTABLE_*`, emits `rustc-cfg`/`rustc-check-cfg`; spawns `freebsd-version` or `emcc` only on those targets; writes no file. |

Every one is the same idiom: a compiler-capability probe that emits `cfg`
flags. None reads the network, none writes outside `OUT_DIR`, none reads
user files, and, on the macOS targets this project builds, none runs anything but the
compiler it was invoked by (`libc`'s spawns are FreeBSD and Emscripten only). The
`unsafe` surface of the graph is the standard library's and `serde_json`'s
(which is `#![deny(unsafe_code)]` except audited SIMD paths), plus
`unicode-width` (none). The workspace's own code is `unsafe_code = "forbid"`.

## Options considered

- **Forbid build scripts outright.** Not possible: `serde` derive, which the
  snapshot and JSON contract need, cannot be used without `proc-macro2`.
- **Allow any build script under an allowed licence.** Rejects the third
  axis the specification added deliberately; a script that fetched or
  patched would pass.
- **Allow-list by crate name, each reviewed here.** Chosen. The list lives
  in `scripts/check-licences.sh` and this ADR; a new crate with a script
  fails the gate until the ADR is extended with what its script does.

## Decision

1. `Unicode-3.0` joins the licence allow-list in `scripts/check-licences.sh`
   and `deny.toml`. The list is otherwise the one in
   `08-distribution-and-project.md`.
2. The crates in the table above (the tenth, `libc`, admitted by ADR-0007 once P/01 is applied) are allowed to carry a build script. The
   allow-list in `scripts/check-licences.sh` names them; a version bump
   within the same major that keeps the script in the same idiom does not
   need a new review, a new crate does.
3. Adding any crate to the allow-list means extending the table above in
   the same change, from a reading of the script, not from its README.

## Consequences

- The gate is green on the foundation pass's dependency set and red the
  moment a new build script appears, which is the intent.
- Reviews are dated: a script can change between versions. `Cargo.lock`
  pins the versions reviewed; a `cargo update` that changes one of these
  crates' major versions is the trigger to re-read.
- No dependency raises the deployment floor: `scripts/check-deployment-target.sh`
  scans every object in every rlib of the release build and asserts
  10.12 / 11.0 on the final artifacts (verified 2026-09-02).

## Compatibility impact

None on users. On contributors: adding a dependency with a build script
means editing this ADR.

## Revisit when

A dependency the product needs carries a script outside the probe idiom
(code generation from network resources, vendored C sources compiled by
`cc`), or a licence outside the list. Either needs its own entry here
before the crate is added.
