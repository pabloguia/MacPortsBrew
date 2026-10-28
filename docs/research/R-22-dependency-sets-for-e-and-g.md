# R-22 — The dependency sets parts E and G would add, and which carry a build script

- **Status:** answered
- **Answered on:** 2026-10-28
- **Machine:** macOS 26.7.1, x86_64; the pinned toolchain 1.98.0. Scratch crates
  only, outside the repository; nothing in the repository's graph changed.
- **Blocking:** work order 16 part P item 2 (the `check-licences.sh` entries P
  stages), part E (the dependency ADR, `sha2`), part G (the lock probe)

## Question

For E's digest (`sha2`, `default-features = false`) and for G's lock probe
(`libc` alone, or a maintained safe wrapper), what is the full transitive set per
target, under which licences, and which crates ship a `build.rs` that
`scripts/check-licences.sh`'s `build_scripts_allowed` would have to admit?

## Method

One scratch crate per candidate, `cargo metadata --format-version 1` and
`cargo tree -e normal,build --target aarch64-apple-darwin` /
`--target x86_64-apple-darwin`. Each crate's source was read from
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/` for its `build.rs` and
features, and its release dates from the crates.io API, both 2026-10-28.

The toolchain: copying `rust-toolchain.toml` into the scratch crate was refused
by the ADR-0002 guard, which matches the file name. The scratch builds selected
the same pinned version with the `RUSTUP_TOOLCHAIN=1.98.0` environment variable
instead. That changes no default, override or installed toolchain, which is what
the guard protects; it is recorded here so the choice is visible.

## Findings

### F-1. `sha2` brings `libc` into the metadata, whatever G chooses

`sha2 = { version = "0.11.1", default-features = false }`:

| Crate | Version | Licence | `build.rs` | Targets |
|---|---|---|---|---|
| sha2 | 0.11.1 | MIT OR Apache-2.0 | no | both |
| digest | 0.11.3 | MIT OR Apache-2.0 | no | both |
| block-buffer | 0.12.1 | MIT OR Apache-2.0 | no | both |
| crypto-common | 0.2.2 | MIT OR Apache-2.0 | no | both |
| hybrid-array | 0.4.15 | MIT OR Apache-2.0 | no | both |
| typenum | 1.20.1 | MIT OR Apache-2.0 | no | both |
| cpufeatures | 0.3.1 | MIT OR Apache-2.0 | no | both |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 | no | both |
| libc | 0.2.191 | MIT OR Apache-2.0 | **yes** | aarch64-apple only, through `cpufeatures`; present in the unfiltered metadata for every target |

`check-licences.sh` reads `cargo metadata` with no target filter, so `libc`'s
build script is seen on every machine. **Part E needs `"libc"` in
`build_scripts_allowed`**, independent of G.

`Sha256::digest` needs no feature: `sha2`'s defaults (`alloc`, `oid`) are
optional, and `Digest::digest` is in `digest-0.11.3/src/digest.rs` lines 56 and
126. Both the one-shot and incremental forms built under
`#![forbid(unsafe_code)]` on both targets, and the `"abc"` vector passed.

### F-2. G's probe needs no `unsafe` with `libc` as constants only

`std::os::unix::fs::OpenOptionsExt::custom_flags` is safe, and `File::metadata`
is `fstat` on the handle. So

```rust
OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(p)?
    .metadata()?.file_type().is_file()
```

compiles under `#![forbid(unsafe_code)]` on both targets, and refuses a symbolic
link with `ELOOP` (the kill for the "drop `O_NOFOLLOW`" mutant). `std` does not
export the constants, and hard-coded values are not acceptable, so `libc` is the
source of three constants and no function is called. **The `forbid` → `deny`
lowering work order 16 E.1 anticipates is then unnecessary**; E's ADR should say
so.

### F-3. The safe wrappers add a crate with a build script each, and still pull `libc`

- `rustix` 1.1.6 (`default-features = false`, `features = ["fs", "std"]`; `fs`
  alone does not accept a `&Path`): adds `rustix` (build script compiling probe
  snippets with `$RUSTC`), `errno`, `bitflags`; `libc` underneath. Licence
  `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT`, accepted through its
  alternatives (cargo-deny not run). Released 2026-10-27.
- `nix` 0.31.3 (`default-features = false`, `features = ["fs"]`): adds `nix`
  (MIT, build script using `cfg_aliases!`), `cfg_aliases`, `bitflags`, `cfg-if`;
  `libc` underneath. Released 2026-05-11.

### F-4. What each build script does

- `libc` 0.2.191: runs `$RUSTC --version` (honouring `RUSTC_WRAPPER`) and emits
  `rustc-cfg`/`rustc-check-cfg`; spawns `freebsd-version` or `emcc` only on those
  targets; writes no file.
- `rustix` 1.1.6: compiles probe snippets from stdin into `OUT_DIR` and deletes
  them; emits `rustc-cfg`.
- `nix` 0.31.3: `cfg_aliases!` only; spawns and writes nothing.

None reads the network. All follow the idiom ADR-0004 already accepted.

## Recommendation

G: option (a), `std` plus `libc` for constants only. It adds no crate beyond what
E's `sha2` already brings, keeps `unsafe_code` at `forbid`, and the open is
exactly what G requires (`std` adds `O_CLOEXEC`). P stages one entry,
`"libc"`, in `build_scripts_allowed`.

## Impact on the specification

None until E's ADR. That ADR records that `libc` enters through `sha2` and that G
calls no `libc` function.
