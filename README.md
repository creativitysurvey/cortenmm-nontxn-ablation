# Artifact: "Gated by Locking Structure, Not Fixed"

This repository accompanies the paper *Gated by Locking Structure,
Not Fixed: The Proof Cost of a Transactional Interface Across Three
Verified Concurrency Protocols in CortenMM*.

**Badges requested:** Artifacts Available, Artifacts Functional,
Results Reproduced.

---

## 1. Requirements

### Hardware
- x86-64 host with KVM support (for the runtime-benchmark steps only;
  the verification steps do not need KVM)
- >= 8 GB RAM, >= 20 GB free disk
- Verification steps are single-threaded; benchmark steps boot a
  1-vCPU guest and do not benefit from more host cores

### Software
- Docker (tested with the `ghcr.io/telos-syslab/cortenmm-artifact-env:v4.1`
  base image, which bundles the pinned Verus toolchain, Rust
  `nightly-2025-02-01`, and Z3)
- `git`, `unzip`
- No GPU, no network access required once the base image is pulled

### Estimated total time
- Kick-the-tires (Section 2): **~10 minutes**
- Full proof-cost reproduction (Section 4, claims 1-4): **~15 minutes**
- Full runtime-benchmark reproduction (Section 4, claims 5-7): **~30-45
  minutes**, dominated by kernel image builds and QEMU boots
- **Total: under 1.5 hours** on the reference hardware described above

---

## 2. Getting Started (kick-the-tires, ~10 minutes)

This step only confirms the environment is set up correctly and one
representative crate verifies; it is not a full reproduction.

```bash
git clone <this-repository-url> artifact-repo
cd artifact-repo

docker pull ghcr.io/telos-syslab/cortenmm-artifact-env:v4.1
docker run -dit --name cortenmm-ae \
  -v "$(pwd):/root/artifact" \
  ghcr.io/telos-syslab/cortenmm-artifact-env:v4.1

docker exec cortenmm-ae bash -c \
  "cd /root/artifact/crates/lock-protocol-coarse && \
   cargo xtask verify --targets lock-protocol-coarse 2>&1 | tail -5"
```

**Expected output** (last line):
```
verification results:: 142 verified, 0 errors
```

(This smoke test uses `lock-protocol-coarse`, not
`lock-protocol-coarse-nontxn`, specifically because the latter has
known residual errors -- see the corrected Section 4, claim 2, and
`docs/verus-tooling-issue.md` -- and is not a suitable first
correctness check.)

If you see this line, the environment is correctly set up and you can
proceed to Section 4. If the crate paths above don't match this
repository's actual layout, see `docs/environment-setup-and-findings.md`
for the exact directory structure used when these results were
produced.

---

## 3. Repository Layout

```
artifact-repo/
|-- crates/           six verified Verus crates (see Section 3.1)
|-- patches/           the exact diffs applied, transactional -> non-transactional
|-- benchmarks/        3 benchmark C programs + raw rdtsc data (CSV)
|-- docs/              environment notes, known issues, tooling limitation writeup
|-- scripts/           re-verification and benchmark-run scripts
`-- paper/             paper source (.tex) and PDF
```

### 3.1 The six crates

| Crate | Role | Baseline or ablated? |
|---|---|---|
| `lock-protocol-rw` | Transactional, fine-grained reader-writer locking | CortenMM's own baseline |
| `lock-protocol-coarse` | Transactional, coarse whole-tree locking | Ablated (locking axis only) |
| `lock-protocol-nontxn` | Non-transactional, fine-grained locking | Ablated (interface axis only) |
| `lock-protocol-coarse-nontxn` | Non-transactional, coarse locking | Ablated (both axes) |
| `lock-protocol-rcu` | Transactional, RCU | CortenMM's own second baseline |
| `lock-protocol-rcu-nontxn` | Non-transactional, RCU | Ablated (interface axis, RCU) |

If your checkout of this repository does not yet contain populated
`crates/*` directories (e.g. you downloaded only the patch-only
scaffold release), see `docs/environment-setup-and-findings.md`
Section "Reconstructing the crates from patches" before proceeding.

---

## 4. Reproducing the Paper's Claims

Each row: the paper's claim, the exact command, the expected output,
and an estimated running time on the reference hardware.

| # | Claim (paper location) | Command | Expected output | Est. time |
|---|---|---|---|---|
| 1 | `CortenMM_rw` verifies at 149 obligations, 0 errors (Table "Proof cost, CortenMM_rw vs. CortenMM_coarse") | `docker exec cortenmm-ae bash -c "cd /root/artifact/crates/lock-protocol-rw && cargo xtask verify --targets lock-protocol-rw 2>&1 \| tail -3"` | `verification results:: 149 verified, 0 errors` | ~2 min |
| 2 | `CortenMM_coarse` and `CortenMM_coarse-nontxn` verify at the SAME obligation count, 142 (the 2x2 matrix's coarse row is identical for both interface styles); `CortenMM_coarse` reaches 0 errors, `CortenMM_coarse-nontxn` has 3 residual errors of the same two tooling-limitation classes documented for `CortenMM_nontxn`/`CortenMM_rcu-nontxn` (see `docs/verus-tooling-issue.md`) | `bash scripts/verify_all_variants.sh` (runs all six; grep for `lock-protocol-coarse` and `lock-protocol-coarse-nontxn` in the output) | `lock-protocol-coarse`: `142 verified, 0 errors`. `lock-protocol-coarse-nontxn`: `142 verified, 3 errors`; full diagnostic in `docs/verification_log_coarse_nontxn.txt` | ~10 min (all six) |
| 3 | `CortenMM_nontxn` verifies at 150 obligations, 2 errors, all residual (Table "Proof cost, ...vs. CortenMM_nontxn") | included in `scripts/verify_all_variants.sh` output | `150 verified, 2 errors`; compare failing clauses against `docs/verus-tooling-issue.md` | (included above) |
| 4 | `CortenMM_rcu-nontxn` verifies at 160 obligations, 3 errors, zero regression from `CortenMM_rcu`'s own 160 (Table "Proof cost, CortenMM_rcu vs. ...") | included in `scripts/verify_all_variants.sh` output | `160 verified, 0 errors` for `lock-protocol-rcu`; `160 verified, 3 errors` for `lock-protocol-rcu-nontxn` | (included above) |
| 5 | `mmap` non-transactional overhead grows from ~0.72x (N=1) to ~13.6x (N=64) (Figure "mmap: batched vs. per-page") | `bash scripts/run_benchmarks.sh`, then at the guest shell: `/test/scale/mmap_batch_scale 1` ... `/test/scale/mmap_batch_scale 64` | `PER_PAGE_OVERHEAD_RATIO` printed by each run; compare against `benchmarks/raw_data_mmap.csv` | ~15 min (kernel build) + ~2 min (runs) |
| 6 | `mprotect` non-transactional overhead reaches up to ~40x at N=64 (Figure "mprotect: batched vs. per-page") | same kernel image as claim 5; at guest shell: `/test/scale/mprotect_scale 4` ... `/test/scale/mprotect_scale 64` | Compare against `benchmarks/raw_data_mprotect.csv` | ~2 min (same image) |
| 7 | Allocator-arena overhead collapses to ~1x once first-touch page faults are included (Table "Allocator-arena workload") | same kernel image; `/test/scale/alloc_arena_scale 4 4`, `16 4`, `64 4`, `64 16` | Compare against `benchmarks/raw_data_alloc_arena.csv` | ~2 min (same image) |
| 8 | `CortenMM_coarse-nontxn` requires zero new proof code in `mod.rs` vs. `CortenMM_nontxn` (Corollary 1) -- this claim is specifically about the `mod.rs` diff, not about the overall verification outcome; the 3 residual errors reported in claim 2 arise from `locking.rs` (`CortenMM_coarse`'s own file, not transplanted) interacting with the transplanted `mod.rs`, not from any new edit to `mod.rs` itself | `diff crates/lock-protocol-nontxn/src/mm/page_table/cursor/mod.rs crates/lock-protocol-coarse-nontxn/src/mm/page_table/cursor/mod.rs` | Empty diff | < 1 min |
| 9 | The `wf_push_level` gap and fix are structurally identical between `CortenMM_rw` and `CortenMM_rcu` (Table "Structural correspondence") | `diff patches/rw-to-nontxn/wf_push_level_fix.md patches/rcu-to-rcu-nontxn/rcu_specific_fixes.md` (read both; the added `forall` clause is line-for-line identical) | Manual inspection | ~5 min |

---

## 5. Known Limitations

- **Claims 2 (coarse-nontxn), 3, and 4 do not reach 0 errors.** This
  is now confirmed to involve **two distinct** documented,
  reproducible Verus tooling limitations, not one: (A) an
  `InstanceId`-equality propagation failure, and (B) a `g_level ==
  level` precondition nondeterminism. Both are described in full,
  with a real, complete diagnostic log for `CortenMM_coarse-nontxn`,
  in `docs/verus-tooling-issue.md`. An earlier version of this
  artifact's documentation (and the paper's own earlier prose)
  described these as "the same issue" in every variant; direct
  reproduction against `CortenMM_coarse-nontxn` showed both issues
  present simultaneously, which is why they are now documented
  separately. The core property each claim is actually about --
  frame preservation, the actual subject of this paper's Research
  Questions 2 and 3 -- **is** fully proved in every case; both issues
  concern only auxiliary bookkeeping facts that Verus fails to
  propagate across a call boundary.
- One asymmetry not stated elsewhere in the paper:
  `CortenMM_nontxn`'s `query_locked` reaches a clean 0-error result;
  `CortenMM_rcu-nontxn`'s does not (all three of its wrapper functions
  hit the residual issue). See `docs/verus-tooling-issue.md`.
- This artifact does not include a build of any external,
  independently maintained Verus system instantiating both sides of
  the interface-atomicity axis (transactional vs. non-transactional),
  since we did not find one during this project (see the paper's
  "Threats to Validity"). A reviewer wishing to test generalization
  beyond CortenMM would need to construct such a system's ablation
  themselves, using this artifact's `patches/` as a template.
- Runtime-benchmark absolute cycle counts (though not the reported
  overhead *ratios*, which are what the paper's claims concern) are
  expected to vary with host CPU model; we report the ratios as the
  reproducible quantity.

---

## 6. Provenance

- Base environment: `ghcr.io/telos-syslab/cortenmm-artifact-env:v4.1`
  (the CortenMM SOSP'25 artifact image)
- `lock-protocol-rw` and `lock-protocol-rcu` are CortenMM's own
  published baselines, included here unmodified for byte-for-byte
  diffing against every ablated variant
- All four ablated crates were constructed by applying the patches in
  `patches/` to copies of the appropriate baseline; see
  `docs/environment-setup-and-findings.md` for the full construction
  history, including four independent artifact-infrastructure issues
  discovered and worked around in the process
