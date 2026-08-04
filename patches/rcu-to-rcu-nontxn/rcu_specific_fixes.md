# Patches specific to CortenMM_rcu -> CortenMM_rcu-nontxn

Applies to: `lock-protocol-rcu-nontxn/src/mm/page_table/cursor/{mod.rs,locking.rs}`

This crate needed the same `wf_push_level` range-widening fix as
`lock-protocol-nontxn` (see `../rw-to-nontxn/wf_push_level_fix.md`),
applied independently to RCU's own `wf_push_level` (which operates
over `Option<PageTableGuard>` rather than the `GuardInPath` enum, but
has the byte-for-byte identical missing-range gap). In addition, RCU's
extra tracked resource (`SubTreeForgotGuard`, threaded by value through
every operation for copy-on-write reclamation) required two further,
RCU-specific patches not needed for the reader-writer-locking variant.

## Patch 1: expose `sub_tree_rt`/`is_root` from `lock_range`

`lock_range`'s original `ensures` clause never related the model's
`sub_tree_rt()` (derived from the lock-protocol token's state machine)
to the cursor's own structure, and never exposed that the returned
`forgot_guards` satisfies `is_root` relative to the locked subtree.
Both facts are needed by every one of `query_locked`/`map_locked`/
`unmap_locked`'s callers to establish `unlock_range`'s precondition.

Added to `lock_range`'s `ensures`:

```rust
res.1@.sub_tree_rt() == res.0.get_guard_level_unwrap(res.0.guard_level).nid(),
res.2@.is_root(res.0.get_guard_level_unwrap(res.0.guard_level).nid()),
```

Both are provable directly from `lock_range`'s existing body -- no new
lemma needed; the underlying construction already establishes them,
they were simply never stated in the postcondition.

## Patch 2: `query`'s own `requires`/`ensures`/loop invariant

Unlike `lock-protocol-nontxn`'s `query` (which only needed the
guard_nid-preservation clause), RCU's `query` additionally threads
`forgot_guards` through every level of descent via
`rec_put_guard_from_path`/`tracked_take`, so it needs an explicit
`is_root` preservation clause too (guard_nid preservation alone is not
sufficient in the RCU case).

Added to `query`'s `requires`:
```rust
forgot_guards@.is_root(
    old(self).get_guard_level_unwrap(old(self).guard_level).nid(),
),
```

Added to `query`'s `ensures`:
```rust
self.get_guard_level_unwrap(self.guard_level).nid() =~= old(
    self,
).get_guard_level_unwrap(old(self).guard_level).nid(),
res.1@.is_root(self.get_guard_level_unwrap(self.guard_level).nid()),
```

Added to the loop invariant (both clauses, mirroring the
requires/ensures above so the loop can re-establish them at every
`continue`):
```rust
self.get_guard_level_unwrap(self.guard_level).nid() =~= old(
    self,
).get_guard_level_unwrap(old(self).guard_level).nid(),
forgot_guards.is_root(
    old(self).get_guard_level_unwrap(old(self).guard_level).nid(),
),
```

`is_root` is monotonic under the set-shrinking `tracked_take`
operation `push_level` performs when descending (removing entries from
a set can only make a `forall`-over-domain property like `is_root`
easier to satisfy, never harder), which is why the loop-invariant
formulation above is sufficient without an additional lemma about
subset monotonicity.

## Result after both patches

160 verified obligations (unchanged from the `CortenMM_rcu` baseline;
zero regression), with the same class of residual `InstanceId`-equality
tooling issue as `CortenMM_nontxn` blocking a clean 0-error result for
`map_locked`/`unmap_locked` (see `../../docs/verus-tooling-issue.md`).
`query_locked` for RCU is NOT fully clean the way `lock-protocol-nontxn`'s
`query_locked` is -- it hits the same residual `InstanceId` issue as
the other two operations, since RCU's `nontxn_api.rs` wrappers all
route through the same `unlock_range` precondition check.
