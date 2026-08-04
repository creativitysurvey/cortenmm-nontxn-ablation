use core::ops::Range;

use vstd::prelude::*;

use common::{
    mm::{Paddr, Vaddr, PagingLevel, page_size},
    mm::page_table::{PageTableConfig, PageTableError},
    mm::page_prop::PageProperty,
    task::DisabledPreemptGuard,
};

use crate::mm::page_table::PageTable;
use crate::mm::page_table::cursor::{CursorMut, PageTableItem, locking, va_range::va_range_wf};
use crate::spec::lock_protocol::LockProtocolModel;

verus! {

// CortenMM_nontxn (Gap 4): each operation independently acquires and
// releases the SAME fine-grained tree-locking protocol used by
// CortenMM_rw (locking::lock_range / locking::unlock_range are entirely
// unmodified, byte-for-byte reused from lock-protocol-rw). What changes
// is only the API surface exposed to callers: instead of a single
// `AddrSpace::lock(range) -> RCursor` handle that stays open across a
// caller-chosen sequence of query/map/unmap calls (the transactional
// design), every operation here is a standalone function that performs
// its own private lock -> operate -> unlock cycle, matching the
// non-transactional, ad-hoc-per-operation locking style the paper
// attributes to Linux (Figure 2, Table 1) rather than CortenMM's own
// design (Figure 4).
//
// This directly tests the paper's claim (S5, "Verification Goals") that
// decoupling concurrency control from operations via the transactional
// interface is what "enables compartmental proof": if that decoupling
// is the reason, removing it (while keeping everything else, including
// the exact same locking protocol, identical) should make verification
// harder or require different proof structure. If instead verification
// cost stays essentially the same, the decoupling claim is not the
// dominant factor -- single-layer abstraction (already tested in Gap 1)
// would remain the load-bearing explanation.
pub fn query_locked<C: PageTableConfig>(
    pt: &PageTable<C>,
    guard: &DisabledPreemptGuard,
    va: &Range<Vaddr>,
    m: Tracked<LockProtocolModel<C>>,
) -> (res: (Result<Option<(Paddr, PagingLevel, PageProperty)>, PageTableError>, Tracked<LockProtocolModel<C>>))
    requires
        pt.wf(),
        va_range_wf::<C>(*va),
        m@.inv(),
        m@.inst_id() == pt.inst@.id(),
        m@.state() is Void,
    ensures
        res.1@.inv(),
        res.1@.inst_id() =~= pt.inst@.id(),
        res.1@.state() is Void,
{
    let (mut cursor, m1) = locking::lock_range(pt, guard, va, m);
    let tracked model = m1.get();
    let result = cursor.query(Tracked(&model));
    let m2 = locking::unlock_range(&mut cursor, Tracked(model));
    (result, m2)
}

pub fn map_locked<C: PageTableConfig>(
    pt: &PageTable<C>,
    guard: &DisabledPreemptGuard,
    va: &Range<Vaddr>,
    item: C::Item,
    m: Tracked<LockProtocolModel<C>>,
) -> (res: (PageTableItem<C>, Tracked<LockProtocolModel<C>>))
    requires
        pt.wf(),
        va_range_wf::<C>(*va),
        C::item_into_raw_spec(item).1 == 1,
        va.start as int + page_size::<C>(C::item_into_raw_spec(item).1) as int <= va.end as int,
        m@.inv(),
        m@.inst_id() == pt.inst@.id(),
        m@.state() is Void,
    ensures
        res.1@.inv(),
        res.1@.inst_id() =~= pt.inst@.id(),
        res.1@.state() is Void,
{
    let (cursor, m1) = locking::lock_range(pt, guard, va, m);
    let mut cursor_mut = CursorMut(cursor);
    let tracked model = m1.get();
    let result = cursor_mut.map(item, Tracked(&model));
    let CursorMut(mut cursor_back) = cursor_mut;
    let m2 = locking::unlock_range(&mut cursor_back, Tracked(model));
    (result, m2)
}

#[verifier::rlimit(40)]
#[verifier::spinoff_prover]
pub fn unmap_locked<C: PageTableConfig>(
    pt: &PageTable<C>,
    guard: &DisabledPreemptGuard,
    va: &Range<Vaddr>,
    len: usize,
    m: Tracked<LockProtocolModel<C>>,
) -> (res: (PageTableItem<C>, Tracked<LockProtocolModel<C>>))
    requires
        pt.wf(),
        va_range_wf::<C>(*va),
        va.start as int + len as int <= va.end as int,
        len > 0 && len % page_size::<C>(1) == 0,
        m@.inv(),
        m@.inst_id() == pt.inst@.id(),
        m@.state() is Void,
    ensures
        res.1@.inv(),
        res.1@.inst_id() =~= pt.inst@.id(),
        res.1@.state() is Void,
{
    let (cursor, m1) = locking::lock_range(pt, guard, va, m);
    let mut cursor_mut = CursorMut(cursor);
    let tracked model = m1.get();
    let result = cursor_mut.take_next(len, Tracked(&model));
    let CursorMut(mut cursor_back) = cursor_mut;
    let m2 = locking::unlock_range(&mut cursor_back, Tracked(model));
    (result, m2)
}

} // verus!
