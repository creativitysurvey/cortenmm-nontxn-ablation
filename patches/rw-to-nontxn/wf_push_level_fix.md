# Patch: strengthen `wf_push_level` to cover the ancestor range

Applies to: `lock-protocol-nontxn/src/mm/page_table/cursor/mod.rs`
(originally identical to `lock-protocol-rw`'s file of the same name).

## Context

`wf_push_level` specifies what `push_level` is allowed to change. The
original clause (identical in `lock-protocol-rw`) only proves that
levels *below* the caller's lock, other than the one `push_level` just
wrote, are unchanged. It says nothing about the ancestor range
`[guard_level, NR_LEVELS]` -- a transactional caller never has to
reason about this range because it never releases the cursor between
operations; a non-transactional caller does, so every wrapper
function's postcondition needs this fact stated explicitly, and the
original code never states it.

## Before

```rust
pub open spec fn wf_push_level(self, post: Self) -> bool {
    &&& post.level == self.level - 1
    &&& post.g_level@ == self.g_level@ - 1
    &&& forall|level: PagingLevel|
        1 <= level <= post.guard_level && level != post.level ==> {
            self.get_guard_level(level) =~= post.get_guard_level(level)
        }
    &&& self.get_guard_level(post.level) is None
    &&& post.get_guard_level(post.level) =~= Some(child_pt)
    &&& self.constant_fields_unchanged(&post)
    &&& self.va == post.va
}
```

## After (added clause)

```rust
    &&& forall|level: PagingLevel|
        #![trigger self.get_guard_level(level)]
        post.guard_level <= level <= C::NR_LEVELS() ==> {
            self.get_guard_level(level) =~= post.get_guard_level(level)
        }
```

This is provable directly from `push_level`'s executable body -- a
single array write, `self.path[(self.level - 1) as usize] =
Some(child_pt)`, strictly below `guard_level` by `push_level`'s own
precondition -- and needs no additional lemma once the range is
stated; Verus's own array-update reasoning discharges it.

## The identical gap in RCU

`lock-protocol-rcu`'s independently written `wf_push_level`, operating
over `Option<PageTableGuard>` rather than `lock-protocol-rw`'s
`GuardInPath` enum, has the byte-for-byte identical gap (missing the
same range). The identical fix (widening the `forall` clause the same
way) resolves it with zero regression to the 160 baseline obligation
count. See `patches/rcu-to-rcu-nontxn/` for the RCU-specific
application of this same patch.

## The step-fact/transitivity pattern (propagating the fact through a loop)

Strengthening `wf_push_level` only makes the fact available
immediately after one `push_level` call. Every operation's loop calls
`push_level` zero or more times across iterations and must carry the
fact from the loop's entry through every iteration to every exit
point. The following two-step pattern, applied at every `push_level`
call site (reproduced verbatim from `query`'s loop body), does this:

```rust
let ghost _cursor = self.0;          // step 0: snapshot loop-top state
...
self.0.push_level(pt);
assert forall|lvl: PagingLevel|      // step 1: single-call fact
    #![trigger _cursor.get_guard_level(lvl)]
    self.0.guard_level <= lvl <= C::NR_LEVELS()
implies {
    _cursor.get_guard_nid(lvl) =~= self.0.get_guard_nid(lvl)
} by {};
assert forall|lvl: PagingLevel|      // step 2: transitivity to old(self)
    #![trigger self.0.get_guard_nid(lvl)]
    self.0.guard_level <= lvl <= C::NR_LEVELS()
implies {
    old(self).0.get_guard_nid(lvl) =~= self.0.get_guard_nid(lvl)
} by {
    assert(old(self).0.get_guard_nid(lvl) =~= _cursor.get_guard_nid(lvl));
    assert(_cursor.get_guard_nid(lvl) =~= self.0.get_guard_nid(lvl));
};
```

Step 1 is discharged automatically from the (now-strengthened)
`wf_push_level` postcondition with an empty proof body. Step 2 chains
this single-call fact to the loop's own invariant via ordinary
transitivity of `=~=`.

## Independent write/exit sites requiring this pattern (CortenMM_nontxn)

| Operation | Site | Pattern instance |
|---|---|---|
| `query` | `push_level` call (single descent) | full (steps 1-2) |
| `map` | `push_level` call (`PageTable` branch) | full |
| `map` | `alloc_if_none` write (`None` branch) | full |
| `map` | post-loop leaf write (`Entry::replace` + `move_forward`) | full |
| `unmap` (`take_next`) | `push_level` call (descend branch) | full |
| `unmap` | `self.0.va = end;` (absent-entry early exit) | step 2 only |
| `unmap` | `move_forward` (absent-entry continue) | full |
| `unmap` | `self.0.va = end;` (empty-child early exit) | step 2 only |
| `unmap` | `move_forward` (empty-child fallthrough) | full |
| `unmap` | post-loop leaf write | full |

9 sites require a new proof (the aggregate count reported in the
paper's Table "Frame-preservation write sites"); ~150 new lines of
proof code across these sites, none of it new executable logic.
