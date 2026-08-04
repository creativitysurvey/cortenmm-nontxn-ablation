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
