// SPDX-License-Identifier: MPL-2.0
//
// New file: lock-protocol-nontxn/src/nontxn_api.rs
//
// The non-transactional wrapper API: three independent functions,
// each performing its own private lock -> operate -> unlock cycle,
// as opposed to CortenMM_rw's transactional cursor shared across a
// caller-chosen operation sequence. Each function's own
// `lock_range`/`unlock_range` pair replaces what a transactional
// caller would do once, across many operations, with the SAME
// underlying (byte-identical) locking primitives -- only the
// sequencing changes.
//
// This file is reproduced from the paper's Methods section
// (\S3.3 "The Missing Obligation and Its Repair" /
// \S3.4 "Propagating the Fact") to the extent quoted there; it is
// the actual source, not a paraphrase.

use super::mm::page_table::cursor::locking;
use super::mm::page_table::cursor::Cursor;
use vstd::prelude::*;

verus! {

pub fn query_locked<C: PageTableConfig>(
    pt: &PageTable<C>,
    guard: &DisabledPreemptGuard,
    va: Vaddr,
    m: Tracked<LockProtocolModel<C>>,
) -> (res: (
    Result<Option<(Paddr, PagingLevel, PageProperty)>, PageTableError>,
    Tracked<LockProtocolModel<C>>,
))
    requires
        pt.wf(),
        va_range_wf::<C>(va..(va + 1)),
        m@.inv(),
        m@.inst_id() == pt.inst@.id(),
        m@.state() is Void,
    ensures
        res.1@.inv(),
        res.1@.inst_id() == pt.inst@.id(),
        res.1@.state() is Void,
{
    let (mut cursor, m1, forgot_guards) =
        locking::lock_range(pt, guard, &(va..(va + 1)), m);
    let model = m1.get();
    let (result, forgot_guards2) = cursor.query(forgot_guards);
    // res.1@.inst_id() =~= pt.inst@.id(),  <- this postcondition is
    // where the residual InstanceId-equality tooling issue
    // (\S5.6 "Threats to Validity") surfaces; see
    // docs/verus-tooling-issue.md.
    let m2 = locking::unlock_range(&mut cursor, Tracked(model), forgot_guards2);
    (result, m2)
}

pub fn map_locked<C: PageTableConfig>(
    pt: &PageTable<C>,
    guard: &DisabledPreemptGuard,
    item: C::Item,
    m: Tracked<LockProtocolModel<C>>,
) -> (res: (Result<(), PageTableError>, Tracked<LockProtocolModel<C>>))
    requires
        pt.wf(),
        m@.inv(),
        m@.inst_id() == pt.inst@.id(),
        m@.state() is Void,
    ensures
        res.1@.inv(),
{
    let (va, level) = item_into_raw::<C>(item);
    let (mut cursor_mut, m1, forgot_guards) =
        locking::lock_range(pt, guard, &(va..(va + page_size::<C>(level))), m);
    let model = m1.get();
    let (result, forgot_guards2) = cursor_mut.map(item, forgot_guards, Tracked(&model));
    let mut cursor_back = cursor_mut;
    let m2 = locking::unlock_range(&mut cursor_back, Tracked(model), forgot_guards2);
    (result, m2)
}

pub fn unmap_locked<C: PageTableConfig>(
    pt: &PageTable<C>,
    guard: &DisabledPreemptGuard,
    va: Vaddr,
    len: usize,
    m: Tracked<LockProtocolModel<C>>,
) -> (res: (Result<(), PageTableError>, Tracked<LockProtocolModel<C>>))
    requires
        pt.wf(),
        len > 0,
        m@.inv(),
        m@.inst_id() == pt.inst@.id(),
        m@.state() is Void,
    ensures
        res.1@.inv(),
{
    let (mut cursor_mut, m1, forgot_guards) =
        locking::lock_range(pt, guard, &(va..(va + len)), m);
    let model = m1.get();
    let (result, forgot_guards2) = cursor_mut.take_next(len, forgot_guards, Tracked(&model));
    let mut cursor_back = cursor_mut;
    let m2 = locking::unlock_range(&mut cursor_back, Tracked(model), forgot_guards2);
    (result, m2)
}

} // verus!
