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

## 4. `#[global_allocator]` conflict: registry-mirror shadowing of local workspace crates (RESOLVED)

### Symptom

`make build` for a kernel tree fails with:

```
error: the `#[global_allocator]` in ostd conflicts with global allocator in: ostd
error: the `#[alloc_error_handler]` in ostd conflicts with allocation error handler in: ostd
```

### Root cause (confirmed)

`osdk/src/base_crate/Cargo.toml.template` (the template `cargo-osdk`
uses to generate the kernel's `run-base` wrapper crate manifest)
specifies `osdk-frame-allocator`, `osdk-heap-allocator`, and `ostd` as
plain `version = "0.16.0"` dependencies, with no `path`. This is only
safe if no matching version is resolvable from the configured registry
mirror. We confirmed, via `find /root/.cargo/registry -iname
'*osdk-frame-allocator*' -o -iname '*osdk-heap-allocator*'`, that both
crates **are** published on the project's registry mirror
(`rsproxy.cn`) at version `0.16.1`, which satisfies the `"0.16.0"`
requirement (Cargo's default caret-range semantics). Once this
mirror's local cache is warm (e.g. from an unrelated earlier `cargo`
invocation), Cargo resolves the version-only dependency to the
**registry-published** copy rather than the local workspace copy that
`aster-nix`/`kernel` depends on directly via a path dependency. This
produces two distinct `ostd` crate instances in the same build, each
defining `#[global_allocator]`/`#[alloc_error_handler]` -- hence the
conflict. This explains why the same build sometimes succeeded and
sometimes failed with no source-code change: it depends entirely on
whether the registry mirror's local cache for these two package names
happens to be warm at build time, which is influenced by unrelated
`cargo` activity in the same environment.

### Fix (confirmed working)

Add explicit `[patch.crates-io]` entries for all three crates to
`osdk/src/base_crate/Cargo.toml.template`, pointing at their local
workspace paths, exactly as the template already does for
`core2`/`libflate`/`libflate_lz77`:

```toml
[patch.crates-io]
osdk-frame-allocator = { path = "<tree-root>/osdk/deps/frame-allocator" }
osdk-heap-allocator = { path = "<tree-root>/osdk/deps/heap-allocator" }
ostd = { path = "<tree-root>/ostd" }
```

After adding this, `cargo-osdk` must be reinstalled from the same
tree's own `osdk/` (`cd osdk && cargo install --path . --force
--locked`) for the patch to take effect, since the template is
compiled into the `cargo-osdk` binary rather than read at build time.
`scripts/run_benchmarks.sh` now applies this patch and reinstall as an
explicit, unconditional step, rather than treating it as a
troubleshooting note.

### Verification

Confirmed on two independent kernel trees (`cortenmm-coarse` and
`cortenmm-rw`) after a full `rm -rf target/`: both build cleanly to a
bootable ISO. `cortenmm-rw`'s image, booted and exercised with all
three benchmark programs, reproduced overhead ratios consistent with
`benchmarks/raw_data_*.csv` (e.g. `mmap_batch_scale 16`: 7.107 vs. the
recorded 7.125-7.562 range; `mprotect_scale 16`: 8.332 vs. the
recorded 8.153; `alloc_arena_scale 16 4`: 1.114 vs. the recorded
1.087), within the paper's documented run-to-run noise band.

## Toolchain versions used

- Rust: pinned nightly (`nightly-2025-02-01`, reports as `rustc
  1.86.0`)
- Verus: pinned to the version bundled with the CortenMM SOSP'25
  artifact image (`ghcr.io/telos-syslab/cortenmm-artifact-env:v4.1`)
- SMT solver: Z3, default `rlimit`
- Guest: QEMU/KVM, 1 vCPU, 8 GiB RAM, `release-lto` build profile

## Reconstructing the crates from patches

If your checkout of this repository does not include populated
`crates/*` directories, reconstruct them as follows. This is the exact
sequence originally used to produce every result reported in the
paper.

```bash
# 1. Obtain the CortenMM SOSP'25 artifact image and start a container
docker pull ghcr.io/telos-syslab/cortenmm-artifact-env:v4.1
docker run -dit --name cortenmm-ae \
  ghcr.io/telos-syslab/cortenmm-artifact-env:v4.1

# 2. lock-protocol-rw and lock-protocol-rcu are CortenMM's own
#    published baselines; they ship inside the artifact image
#    unmodified at /root/asterinas/verification/.

# 3. CortenMM_coarse: apply the Gap-1 locking-granularity change
#    (simplify locking.rs / va_range.rs to a single whole-tree lock;
#    mod.rs is left byte-for-byte identical to lock-protocol-rw's).

# 4. CortenMM_nontxn: copy lock-protocol-rw, then apply
#    patches/rw-to-nontxn/wf_push_level_fix.md's diff to mod.rs and
#    locking.rs, and add patches/rw-to-nontxn/nontxn_api.rs as a new
#    file.

# 5. CortenMM_coarse-nontxn: copy lock-protocol-coarse, then
#    transplant lock-protocol-nontxn's mod.rs onto it verbatim (no
#    further edit -- this zero-edit transplant, and its resulting
#    142-obligation, 0-new-proof-code outcome, is itself one of the
#    paper's checkable claims, see README.md Section 4, claim 8).

# 6. CortenMM_rcu-nontxn: copy lock-protocol-rcu, then apply
#    patches/rw-to-nontxn/wf_push_level_fix.md's fix (same clause,
#    applied to RCU's own wf_push_level) plus the two RCU-specific
#    patches in patches/rcu-to-rcu-nontxn/rcu_specific_fixes.md, and
#    add an RCU-specific nontxn_api.rs (structurally analogous to
#    patches/rw-to-nontxn/nontxn_api.rs, but routing through
#    locking::lock_range's 3-tuple return and threading
#    SubTreeForgotGuard through every call).

# 7. Verify each crate independently:
bash scripts/verify_all_variants.sh
```

