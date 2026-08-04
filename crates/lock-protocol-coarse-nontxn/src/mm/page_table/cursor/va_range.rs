use std::ops::Range;

use vstd::prelude::*;

use common::mm::{Vaddr, PagingLevel, MAX_USERSPACE_VADDR};
use common::mm::page_table::{PageTableConfig, PagingConstsTrait, pte_index_spec};
use common::spec::{common::*, node_helper::self};

verus! {

pub open spec fn va_range_wf<C: PageTableConfig>(va: Range<Vaddr>) -> bool {
    &&& valid_va_range::<C>(va)
    &&& va.start < va.end < MAX_USERSPACE_VADDR
    &&& vaddr_is_aligned::<C>(va.start)
    &&& vaddr_is_aligned::<C>(va.end)
}

// CortenMM_coarse: the "covering PT page" for ANY va range is always the
// root node, since a single global lock is acquired for every transaction
// regardless of the requested range. This collapses the tree-traversal-based
// guard-level computation in CortenMM_rw/CortenMM_adv into a constant.
pub open spec fn va_range_get_guard_level<C: PageTableConfig>(va: Range<Vaddr>) -> PagingLevel
    recommends
        va_range_wf::<C>(va),
{
    C::NR_LEVELS_SPEC()
}

pub proof fn lemma_va_range_get_guard_level<C: PageTableConfig>(va: Range<Vaddr>)
    requires
        va_range_wf::<C>(va),
    ensures
        1 <= va_range_get_guard_level::<C>(va) <= C::NR_LEVELS_SPEC(),
{
    C::lemma_consts_properties();
}

pub open spec fn va_range_get_tree_path<C: PageTableConfig>(va: Range<Vaddr>) -> Seq<NodeId>
    recommends
        va_range_wf::<C>(va),
{
    Seq::new(
        (C::NR_LEVELS_SPEC() + 1 - va_range_get_guard_level::<C>(va)) as nat,
        |i| va_level_to_nid::<C>(va.start, (C::NR_LEVELS_SPEC() - i) as PagingLevel),
    )
}

pub proof fn lemma_va_range_get_tree_path<C: PageTableConfig>(va: Range<Vaddr>)
    requires
        va_range_wf::<C>(va),
    ensures
        va_range_get_tree_path::<C>(va).all(|nid| node_helper::valid_nid::<C>(nid)),
        va_range_get_tree_path::<C>(va).len() == C::NR_LEVELS_SPEC() + 1
            - va_range_get_guard_level::<C>(va),
{
    lemma_va_range_get_guard_level::<C>(va);
    assert forall|i: int| 0 <= i < va_range_get_tree_path::<C>(va).len() implies {
        #[trigger] node_helper::valid_nid::<C>(va_range_get_tree_path::<C>(va)[i])
    } by {
        lemma_va_level_to_nid_valid::<C>(va.start, (C::NR_LEVELS_SPEC() - i) as PagingLevel);
    }
}

} // verus!
