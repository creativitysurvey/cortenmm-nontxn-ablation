# Environment setup notes and known infrastructure findings

This document records four independent, reproducible
artifact-infrastructure issues discovered in the course of this
project, each documented here for the CortenMM/Asterinas/Verus/OSDK
maintainers, independently of the paper's main claims (per the paper's
Introduction, "Contributions" list, final bullet).

## 1. Cross-tree build-path pollution in `cargo-osdk`

**Symptom**: `error: package collision in the lockfile: packages
align_ext v0.1.0 (.../cortenmm-B/ostd/libs/align_ext) and align_ext
v0.1.0 (.../cortenmm-A/ostd/libs/align_ext) are different, but only
one can be written to lockfile unambiguously` when building a kernel
tree that was created by `cp -r`-ing another tree.

**Root cause**: `osdk/src/base_crate/Cargo.toml.template` contained a
hardcoded *absolute* path (`/root/asterinas/cortenmm-rw/third_party/...`)
in its `[patch.crates-io]` section rather than a relative one. Any
kernel tree built from a copy of another tree inherits this absolute
path pointing at the *original* tree, causing Cargo to see two
different `align_ext` packages at build time.

**Fix**: change the template's `[patch.crates-io]` paths from
absolute to relative (`../../../third_party/...`), in every kernel
tree independently.

**A second-order consequence**: `cargo-osdk` is typically installed
once, globally, to `~/.cargo/bin/cargo-osdk`, and is *not*
automatically rebuilt when its own source is edited. After applying
the template fix above, you must explicitly reinstall it:

```
cd <tree>/osdk && cargo install --path . --force --locked
```

(`--locked` matters: without it, `cargo install` may re-resolve
dependencies and pick up an incompatible newer `libflate` version that
the kernel build's own vendored-patch mechanism does not reach, since
`cargo install` uses `osdk`'s own `Cargo.toml`/`Cargo.lock`, not the
kernel's generated one.)

## 2. Guest-runtime startup fault for non-`pthread`-linked static binaries

**Symptom**: any statically-linked C test program that does not link
`pthread` -- including, in the minimal case, `int main(void){return
0;}` with zero library calls -- crashes at startup with `malloc():
unaligned tcache chunk detected` / `Aborted` (exit code 134) on this
guest environment, *before* reaching any user code (confirmed via a
program that prints nothing before `main`'s single `printf`, which
never executes).

**Workaround**: link `pthread` and run the actual work inside a
spawned thread (even a single, trivial `pthread_create`/`pthread_join`
pair with an empty thread body resolves the crash). All three
benchmark programs in `benchmarks/` use this workaround.

**Status**: root cause not identified (plausibly a single-threaded
vs.\ multi-threaded initial-thread/TLS-setup code path in the guest
runtime that has never been exercised, since every pre-existing test
program in the base artifact happens to be multi-threaded already).
Not filed upstream as part of this project.

## 3. Corrected misdiagnosis: `mprotect()` is not independently broken

An earlier version of `mprotect_scale.c`, written and tested *before*
issue #2 above was diagnosed, crashed with the same
`malloc(): unaligned tcache chunk detected` symptom and was initially
misattributed to a `mprotect()`-specific kernel bug. Re-testing with
the `pthread` workaround (issue #2) showed `mprotect()` works
correctly; the crash was entirely explained by issue #2, not by
anything specific to `mprotect()`. The corrected `mprotect_scale.c` in
`benchmarks/` includes a comment recording this history.

## 4. Manifest-generation inconsistency across `cargo-osdk` builds

We observed that two builds of `cargo-osdk` from ostensibly identical
source (`osdk/src` was confirmed byte-for-byte identical via `diff -rq`
across two kernel trees) produced *semantically different* generated
`Cargo.toml` manifests for the kernel's own `run-base` wrapper crate:
one build emitted explicit `path = "..."` dependencies for
`osdk-frame-allocator`/`osdk-heap-allocator`/`ostd`; a later build
(after the fix in issue #1 and a fresh `cargo install`) emitted
version-only dependencies (no `path` field) for the same three
crates, which in turn caused a `#[global_allocator]`/
`#[alloc_error_handler]` "conflicts with ostd" compile error not
present in the earlier build. We did not root-cause this (it would
require reading `cargo-osdk`'s own manifest-generation logic, which we
did not do), and report it here as an open, reproducible finding for
the OSDK maintainers rather than as something this artifact resolves.

## Toolchain versions used

- Rust: pinned nightly (`nightly-2025-02-01`, reports as `rustc
  1.86.0`)
- Verus: pinned to the version bundled with the CortenMM SOSP'25
  artifact image (`ghcr.io/telos-syslab/cortenmm-artifact-env:v4.1`)
- SMT solver: Z3, default `rlimit`
- Guest: QEMU/KVM, 1 vCPU, 8 GiB RAM, `release-lto` build profile
