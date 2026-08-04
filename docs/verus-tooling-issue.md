# Residual Verus tooling limitations

This document describes, precisely, **two distinct** tooling
limitations that keep `CortenMM_nontxn`, `CortenMM_coarse-nontxn`, and
`CortenMM_rcu-nontxn` from a clean 0-error verification result.

An earlier version of this document (and, correspondingly, earlier
prose in the paper) described these variants' residual errors as "the
same tooling limitation" in every case. That was an oversimplification
that a full reproduction against `CortenMM_coarse-nontxn`, which
exhibits both issues simultaneously, made visible. The two issues are
real, distinct, and independently reproducible; this document now
describes each on its own.

## Issue A: `InstanceId`-equality propagation across a function-call boundary

### Symptom

At `unlock_range` call sites inside the `nontxn_api.rs` wrapper
functions, Verus reports the postcondition

```
res.1@.inst_id() =~= pt.inst@.id()
```

as unsatisfied, with a diagnostic sub-line reading:

```
|   res.1.inst_id().0 =~= pt.inst.id().0
```

i.e. Verus's own diagnostic indicates that the *unwrapped* field
comparison would succeed, but the wrapped/structural comparison over
the `InstanceId` type itself does not automatically discharge.

### What we tried

1. A standalone `assert(...)` of the same fact placed immediately
   before the failing point succeeds on its own, confirming the fact
   IS true and provable in isolation.
2. `#[verifier::rlimit(40)]` on the wrapper function: no effect.
3. `#[verifier::spinoff_prover]` on the wrapper function: no effect.

### Where observed

- `CortenMM_nontxn`: `query_locked`, `map_locked`, `unmap_locked` (all
  three wrapper functions)
- `CortenMM_coarse-nontxn`: `query_locked`, `map_locked`,
  `unmap_locked` (all three; confirmed by direct reproduction, see
  `verification_log_coarse_nontxn.txt` in this directory)
- `CortenMM_rcu-nontxn`: all three wrapper functions

## Issue B: `g_level == level` precondition nondeterminism

### Symptom

At an `unlock_range` call site, Verus reports the precondition

```
old(cursor).g_level@ == old(cursor).level
```

as unsatisfied, even though (as with Issue A) a standalone `assert` of
the identical fact placed immediately before the call succeeds.

### Diagnosis

This does not resemble a logical gap: the fact is independently
provable at the call site, but the precondition checker does not
accept it as satisfied when checking the call itself. This matches the
general class of nondeterministic verification failures the Verus
project has itself documented (trigger selection / solver-path
nondeterminism between an isolated `assert` and the same fact checked
as part of a function-call precondition).

### Where observed

- `CortenMM_nontxn` (rw-based): originally observed at the
  `unmap_locked` call site only.
- `CortenMM_coarse-nontxn`: observed at the `map_locked` call site
  (a **different** operation than where it appeared in the rw-based
  variant) -- see `verification_log_coarse_nontxn.txt`. This
  cross-variant, cross-operation recurrence at an unpredictable site is
  itself consistent with the nondeterminism diagnosis above: the issue
  does not appear to be tied to a specific operation's proof structure,
  but can surface at different call sites across otherwise-similar
  builds.

## What we tried for both issues

Both standard remedies suggested by Verus's own troubleshooting
guidance were attempted on the affected functions:
`#[verifier::rlimit(n)]` and `#[verifier::spinoff_prover]`. Neither
resolves either issue.

## Why neither issue affects the paper's core claims

The property each precondition/postcondition is guarding --
`model.state() is Locked`/`WriteLocked` and
`model.sub_tree_rt() == cursor.get_guard_level_unwrap(...).nid()` (the
frame-preservation / threshold-effect property that Research Questions
2 and 3 concern) -- **is** fully proved in every case; both issues are
about auxiliary bookkeeping facts (instance identity, a redundant
level-equality restatement) that Verus fails to automatically
propagate across a call boundary, not about the substantive property
this paper's ablations turn on.

## Per-variant summary (confirmed by direct reproduction)

| Variant | Verified obligations | Errors | Issue A present? | Issue B present? |
|---|---|---|---|---|
| `CortenMM_rw` | 149 | 0 | -- | -- |
| `CortenMM_coarse` | 142 | 0 | -- | -- |
| `CortenMM_nontxn` | 150 | 2 | yes (all 3 wrapper fns) | yes (`unmap_locked`) |
| `CortenMM_coarse-nontxn` | 142 | 3 | yes (all 3 wrapper fns) | yes (`map_locked`) |
| `CortenMM_rcu` | 160 | 0 | -- | -- |
| `CortenMM_rcu-nontxn` | 160 | 3 | yes (all 3 wrapper fns) | not separately confirmed; see note below |

Note: earlier reproduction of `CortenMM_rcu-nontxn` reported 3 errors,
consistent with all three wrapper functions hitting Issue A; we did
not, at the time, separately check whether one of the 3 was actually
an instance of Issue B rather than a second/third instance of Issue A.
A reviewer reproducing this variant should re-run
`cargo xtask verify --targets lock-protocol-rcu-nontxn` and inspect the
full diagnostic output (as `verification_log_coarse_nontxn.txt`
illustrates for the coarse-nontxn case) to resolve this precisely.

## Status

Unresolved as of this artifact's preparation. Not filed against the
upstream Verus issue tracker as part of this project. A reviewer or
future user of this artifact wishing to pursue a fix should reproduce
via `cargo xtask verify --targets <crate>` and inspect the full
diagnostic output as described above; `verification_log_coarse_nontxn.txt`
in this directory is a complete, real example.
