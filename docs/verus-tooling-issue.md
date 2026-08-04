# The residual `InstanceId`-equality Verus tooling issue

This document describes, precisely, the single tooling limitation that
keeps `CortenMM_nontxn` and `CortenMM_rcu-nontxn` from a clean 0-error
verification result, referenced throughout the paper (Experimental
Evaluation \S "Threats to Validity", Theoretical Analysis \S "Scope and
Non-Claims", Introduction's contributions list).

## Symptom

At every `unlock_range` call site inside the three `nontxn_api.rs`
wrapper functions (`query_locked`, `map_locked`, `unmap_locked`), Verus
reports the precondition

```
model.inst_id() == cursor.inst.id()
```

as unsatisfied, with a diagnostic sub-line reading:

```
|   model.inst_id().0 == cursor.inst.id().0
```

i.e. Verus's own diagnostic indicates that the *unwrapped* field
comparison would succeed, but the wrapped/structural comparison
(`=~=` over the `InstanceId` type itself, which nests a field of type
`InstanceId.0` inside it) does not automatically discharge.

## What we tried

1. A standalone `assert(model.inst_id() == cursor.inst.id());`
   placed immediately before the failing call succeeds on its own,
   confirming the fact IS true and provable in isolation -- it is
   Verus's automatic propagation of this specific fact across the
   function-call boundary that fails, not the fact itself.
2. `#[verifier::rlimit(40)]` on the wrapper function: no effect.
3. `#[verifier::spinoff_prover]` on the wrapper function: no effect.

Both (2) and (3) are the standard remedies Verus's own documentation
suggests for SMT-solver nondeterminism/timeout-adjacent failures; that
neither resolves this specific failure, combined with the standalone
`assert` succeeding, is why we diagnose this as a structural-equality
/ trigger-propagation limitation in Verus's handling of nested
`InstanceId` values specifically, rather than an underlying logical
gap or a solver-timeout issue.

## Why this does not affect the paper's core claims

The property this precondition is guarding -- `model.state() is
Locked` and `model.sub_tree_rt() == cursor.get_guard_level_unwrap(...).nid()`
-- is the actual subject of the paper's Research Questions 2 and 3
(the frame-preservation / threshold-effect claims), and **is** fully
proved in every case; the diagnostic tables reproduced in
`benchmarks/verification_obligations_summary.csv` and the paper's
Experimental Evaluation show every other clause of every precondition
at every one of these call sites succeeding (`✔`), with only this one
`InstanceId`-equality clause failing (`✘`).

## Status

Unresolved as of this artifact's preparation. We did not file this
against the upstream Verus issue tracker as part of this project; a
reviewer or future user of this artifact wishing to pursue a fix
should reproduce the failure via `cargo xtask verify --targets
lock-protocol-nontxn` (or `lock-protocol-rcu-nontxn`) and inspect the
full diagnostic output as described above.
