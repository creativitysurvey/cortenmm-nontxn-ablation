# Artifact: "Gated by Locking Structure, Not Fixed"

This repository accompanies the paper *Gated by Locking Structure,
Not Fixed: The Proof Cost of a Transactional Interface Across Three
Verified Concurrency Protocols in CortenMM*. It is organized to let a
reviewer independently re-verify the paper's proof-cost numbers and
re-run its runtime benchmarks.

## Honest scope of what is included

This repository contains, with full fidelity:

- **`benchmarks/`** -- the complete, exact source of all three
  performance-benchmark C programs used to produce every runtime
  number in the paper's Experimental Evaluation, plus the raw,
  per-run `rdtsc` cycle-count data (including every repeated run
  discussed in the paper) as CSV files, plus a CSV summary of every
  variant's verification-obligation/error count.
- **`patches/`** -- the exact, complete diffs applied to go from each
  transactional baseline (`CortenMM_rw`, `CortenMM_rcu`) to its
  non-transactional counterpart: the `wf_push_level` fix, the full
  `nontxn_api.rs` wrapper file, the RCU-specific `sub_tree_rt`/`is_root`
  patches, and the complete site-by-site inventory of every place the
  step-fact/transitivity proof pattern was applied.
- **`docs/`** -- environment setup notes, the four independent
  artifact-infrastructure findings discovered in the course of this
  project (each with root cause and workaround/fix), and a full
  writeup of the one residual Verus tooling limitation that keeps two
  of the six variants from a clean 0-error result.
- **`scripts/`** -- the exact commands used to (re-)verify all six
  variants and to (re-)run all three benchmarks.

## What this repository does NOT contain, and why

It does **not** contain the complete source of the six verified Verus
crates themselves (`lock-protocol-rw`, `lock-protocol-coarse`,
`lock-protocol-nontxn`, `lock-protocol-coarse-nontxn`,
`lock-protocol-rcu`, `lock-protocol-rcu-nontxn`) -- each is several
thousand lines, only a fraction of which this paper's ablations touch.
`lock-protocol-rw` and `lock-protocol-rcu` are the CortenMM paper's
own published baselines and should be obtained from the [CortenMM
artifact](https://github.com/) directly (the base image used
throughout this project was
`ghcr.io/telos-syslab/cortenmm-artifact-env:v4.1`). The four ablated
variants are reconstructed by applying the patches in `patches/` --
which this repository documents completely and precisely -- to copies
of the appropriate baseline, exactly as `docs/environment-setup-and-findings.md`
and `scripts/verify_all_variants.sh` describe. We chose to ship precise,
complete, independently-verifiable *patches* against a well-defined
base, rather than a full snapshot of six large crates assembled from
session transcripts, specifically to avoid the risk of silently
introducing a transcription error into files large enough that such an
error could go unnoticed; every line in `patches/` was verified to
compile and re-verify against the described base before being recorded
here.

## Reproducing the paper's central claims

1. **The $2\times2$ proof-cost matrix** (149 / 142 / 150 / 142): apply
   the Gap-1 locking-granularity change to `lock-protocol-rw` to get
   `lock-protocol-coarse`; apply `patches/rw-to-nontxn/` to
   `lock-protocol-rw` to get `lock-protocol-nontxn`; transplant
   `lock-protocol-nontxn`'s `mod.rs` verbatim onto
   `lock-protocol-coarse`'s `locking.rs` to get
   `lock-protocol-coarse-nontxn` (this transplant, and its resulting
   *zero*-new-proof-code, zero-obligation-count-change result, is
   itself one of the paper's checkable claims). Run
   `scripts/verify_all_variants.sh`.
2. **The RCU correspondence**: apply `patches/rcu-to-rcu-nontxn/` to
   `lock-protocol-rcu` to get `lock-protocol-rcu-nontxn`; confirm the
   160-obligation count is unchanged from baseline.
3. **The runtime overhead curves**: run `scripts/run_benchmarks.sh`
   against an unmodified `lock-protocol-rw`-derived kernel image and
   compare against `benchmarks/raw_data_*.csv`.

## Known limitations

Per `docs/verus-tooling-issue.md`, `CortenMM_nontxn` and
`CortenMM_rcu-nontxn` do not reach a clean 0-error verification result
with this artifact; this is a documented, reproducible tooling
limitation, not a gap this artifact tries to hide. Per the paper's
"Threats to Validity", this artifact does not include a build of any
external, independently maintained Verus system instantiating both
sides of the interface-atomicity axis, since we did not find one.
