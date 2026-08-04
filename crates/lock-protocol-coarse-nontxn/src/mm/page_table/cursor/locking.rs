use std::mem::ManuallyDrop;
use core::ops::Deref;
use std::ops::Range;

use vstd::invariant;
use vstd::prelude::*;
use vstd::seq::axiom_seq_update_different;
use vstd::atomic_with_ghost;
use vstd::bits::*;
use vstd::rwlock::{ReadHandle, WriteHandle};
use vstd::vpanic;
use vstd::pervasive::allow_panic;
use vstd::pervasive::unreached;

use vstd_extra::manually_drop::*;

use common::{
    mm::{Paddr, Vaddr, PagingLevel, page_size},
    mm::page_table::{PageTableConfig, PagingConstsTrait, pte_index, pte_index_spec},
    spec::{common::*, node_helper::self},
    task::DisabledPreemptGuard,
};

use crate::mm::page_table::PageTable;
use crate::mm::page_table::node::{
    PageTableNode, PageTableNodeRef, PageTableReadLock, PageTableWriteLock,
    child::{Child, ChildRef},
    entry::Entry,
    rwlock::{PageTablePageRwLock, RwWriteGuard, RwReadGuard},
};
use crate::mm::page_table::cursor::{MAX_NR_LEVELS, GuardInPath, Cursor, va_range::*};
use crate::spec::{
    rw::{lemma_wf_tree_path_inc, SpecInstance},
    lock_protocol::LockProtocolModel,
};

verus! {

// CortenMM_coarse: single global write lock on the root PT page, regardless
// of the requested va range. The tree-traversal-based covering-page search
// used by CortenMM_rw/CortenMM_adv is entirely removed; the covering page is
// always the root by construction (va_range_get_guard_level is constant).
#[verifier::exec_allows_no_decreases_clause]
pub fn lock_range<'a, C: PageTableConfig>(
    pt: &'a PageTable<C>,
    guard: &'a DisabledPreemptGuard,
    va: &Range<Vaddr>,
    m: Tracked<LockProtocolModel<C>>,
) -> (res: (Cursor<'a, C>, Tracked<LockProtocolModel<C>>))
    requires
        pt.wf(),
        va_range_wf::<C>(*va),
        m@.inv(),
        m@.inst_id() == pt.inst@.id(),
        m@.state() is Void,
    ensures
        res.0.wf(),
        res.0.wf_init_state(*va),
        res.0.inst@.id() == pt.inst@.id(),
        res.1@.inv(),
        res.1@.inst_id() == pt.inst@.id(),
        res.1@.state() is WriteLocked,
        res.1@.path() =~= va_range_get_tree_path::<C>(*va),
        res.0.wf_with_lock_protocol_model(res.1@),
{
    let mut path: [GuardInPath<C>; MAX_NR_LEVELS] = [
        GuardInPath::<C>::Unlocked,
        GuardInPath::<C>::Unlocked,
        GuardInPath::<C>::Unlocked,
        GuardInPath::<C>::Unlocked,
        GuardInPath::<C>::Unlocked,
    ];

    proof {
        assert(C::NR_LEVELS() <= MAX_NR_LEVELS) by {
            C::lemma_consts_properties();
        }
    }

    let cur_pt = pt.root.borrow();

    let tracked mut m = m.get();
    proof {
        m.token = pt.inst.borrow().locking_start(m.cpu, m.token);
        assert(m.state() is ReadLocking);
    }

    proof {
        lemma_va_range_get_guard_level::<C>(*va);
        lemma_va_range_get_tree_path::<C>(*va);
        node_helper::lemma_root_id::<C>();
        assert(cur_pt.deref().level_spec() == C::NR_LEVELS());
        assert(m.path().len() == 0);
        lemma_wf_tree_path_inc::<C>(m.path(), cur_pt.deref().nid@);
        // Connect the root node's nid to the level-based computation used by
        // va_range_get_tree_path, so we can later show the two singleton
        // sequences (the actual lock path vs. the spec'd tree path) coincide.
        reveal(node_helper::trace_to_nid_rec);
        assert(cur_pt.deref().nid@ == va_level_to_nid::<C>(va.start, C::NR_LEVELS_SPEC()));
    }

    let cur_level = cur_pt.deref().level();
    let res = cur_pt.lock_write(guard, Tracked(m));
    let cur_pt_wlockguard = res.0;
    proof {
        m = res.1.get();
    }
    path[cur_level as usize - 1] = GuardInPath::Write(cur_pt_wlockguard);

    proof {
        // res.1@.path() == m.path().push(root_nid) == seq![root_nid] (m.path() was empty),
        // and va_range_get_tree_path::<C>(*va) is a length-1 sequence whose only element is
        // va_level_to_nid(va.start, NR_LEVELS) == root_nid (established above). Both sides
        // are therefore the same singleton sequence.
        assert(va_range_get_tree_path::<C>(*va).len() == 1);
        assert(va_range_get_tree_path::<C>(*va)[0]
            == va_level_to_nid::<C>(va.start, C::NR_LEVELS_SPEC()));
        assert(m.path() =~= va_range_get_tree_path::<C>(*va));
    }

    let tracked inst = pt.inst.borrow().clone();
    let cursor = Cursor::<'a, C> {
        path,
        preempt_guard: guard,
        level: cur_level,
        guard_level: cur_level,
        va: va.start,
        barrier_va: va.start..va.end,
        inst: Tracked(inst),
        g_level: Ghost(cur_level),
    };
    assert(cursor.wf()) by {
        admit();
    };
    assert(C::BASE_PAGE_SIZE_SPEC() == page_size::<C>(1)) by {
        admit();
    };  // TODO

    (cursor, Tracked(m))
}

pub fn unlock_range<C: PageTableConfig>(
    cursor: &mut Cursor<'_, C>,
    m: Tracked<LockProtocolModel<C>>,
) -> (res: Tracked<LockProtocolModel<C>>)
    requires
        old(cursor).wf(),
        old(cursor).g_level@ == old(cursor).level,
        old(cursor).wf_with_lock_protocol_model(m@),
        m@.inv(),
        m@.state() is WriteLocked,
    ensures
        cursor.g_level@ == C::NR_LEVELS() + 1,
        forall|level: PagingLevel|
            #![trigger cursor.get_guard_level(level)]
            1 <= level <= C::NR_LEVELS() ==> cursor.get_guard_level(level) is Unlocked,
        res@.inv(),
        res@.state() is Void,
{
    proof {
        C::lemma_consts_properties();
    }

    let tracked mut m = m.get();

    let mut cur_level = cursor.level;
    let ghost level = cursor.level;
    let ghost guard_level = cursor.guard_level;
    while cur_level < cursor.guard_level
        invariant
            cursor.level <= cur_level <= cursor.guard_level,
            m.inv(),
            m.inst_id() == cursor.inst@.id(),
            m.state() is WriteLocked,
            cursor.wf(),
            cursor.wf_with_lock_protocol_model(m),
            cursor.g_level@ == cur_level,
            cursor.level == level,
            cursor.guard_level == guard_level,
        decreases cursor.guard_level - cur_level,
    {
        let ghost _cursor = *cursor;

        assert(cursor.get_guard_level(cur_level) is ImplicitWrite);
        proof {
            cursor.lemma_take_guard_sound((cur_level - 1) as usize);
            cursor.lemma_take_guard_sustain_wf((cur_level - 1) as usize);
        }
        let GuardInPath::ImplicitWrite(guard) = cursor.take_guard(cur_level as usize - 1) else {
            unreached()
        };
        assert(cursor.wf_path());
        assert(cursor.wf_with_lock_protocol_model(m)) by {
            assert(cursor.g_level@ <= cursor.guard_level);
            assert forall|level: PagingLevel|
                #![trigger cursor.get_guard_level(level)]
                cursor.guard_level@ <= level <= C::NR_LEVELS() implies {
                &&& cursor.get_guard_level(level) !is Unlocked
                &&& match cursor.get_guard_level(level) {
                    GuardInPath::Read(rguard) => m.path()[C::NR_LEVELS() - level] == rguard.nid(),
                    GuardInPath::Write(wguard) => m.path()[C::NR_LEVELS() - level] == wguard.nid(),
                    GuardInPath::ImplicitWrite(wguard) => true,
                    GuardInPath::Unlocked => true,
                }
            } by {
                assert(cursor.get_guard_level(level) =~= _cursor.get_guard_level(level)) by {
                    assert(cursor.path@ =~= _cursor.path@.update(
                        cur_level - 1,
                        GuardInPath::Unlocked,
                    ));
                    assert(cursor.path@[level - 1] =~= _cursor.path@[level - 1]) by {
                        axiom_seq_update_different(
                            _cursor.path@,
                            level - 1,
                            cur_level - 1,
                            GuardInPath::Unlocked,
                        );
                    };
                };
            };
        };
        // This is implicitly write locked. Don't drop (unlock) it.
        let _ = ManuallyDrop::new(guard);
        cur_level += 1;
        assert(cursor.g_level@ == cur_level);
    }

    let ghost _cursor = *cursor;

    let guard_level = cursor.guard_level;
    assert(cursor.get_guard_level(guard_level) is Write);
    proof {
        cursor.lemma_take_guard_sound((guard_level - 1) as usize);
        cursor.lemma_take_guard_sustain_wf((guard_level - 1) as usize);
    }
    let GuardInPath::Write(mut guard_node) = cursor.take_guard(guard_level as usize - 1) else {
        unreached()
    };
    let res = guard_node.drop(Tracked(m));
    proof {
        m = res.get();
    }

    assert(cursor.wf());
    assert(cursor.wf_with_lock_protocol_model(m)) by {
        assert(cursor.g_level@ == cursor.guard_level + 1);
        assert forall|level: PagingLevel|
            #![trigger cursor.get_guard_level(level)]
            cursor.g_level@ <= level <= C::NR_LEVELS() implies {
            &&& cursor.get_guard_level(level) !is Unlocked
            &&& match cursor.get_guard_level(level) {
                GuardInPath::Read(rguard) => m.path()[C::NR_LEVELS() - level] == rguard.nid(),
                GuardInPath::Write(wguard) => m.path()[C::NR_LEVELS() - level] == wguard.nid(),
                GuardInPath::ImplicitWrite(wguard) => true,
                GuardInPath::Unlocked => true,
            }
        } by {
            assert(cursor.get_guard_level(level) =~= _cursor.get_guard_level(level)) by {
                assert(cursor.path@ =~= _cursor.path@.update(
                    guard_level - 1,
                    GuardInPath::Unlocked,
                ));
                assert(cursor.path@[level - 1] =~= _cursor.path@[level - 1]) by {
                    axiom_seq_update_different(
                        _cursor.path@,
                        level - 1,
                        guard_level - 1,
                        GuardInPath::Unlocked,
                    );
                };
            };
        };
    };

    let mut cur_level = guard_level + 1;
    while cur_level <= C::NR_LEVELS()
        invariant
            guard_level + 1 <= cur_level <= C::NR_LEVELS() + 1,
            cur_level == cursor.g_level@,
            m.inv(),
            m.state() is ReadLocking,
            cursor.wf(),
            cursor.wf_with_lock_protocol_model(m),
            cursor.level == level,
            cursor.guard_level == guard_level,
        decreases C::NR_LEVELS() + 1 - cur_level,
    {
        let ghost _cursor = *cursor;

        assert(cursor.get_guard_level(cur_level) is Read);
        proof {
            cursor.lemma_take_guard_sound((cur_level - 1) as usize);
            cursor.lemma_take_guard_sustain_wf((cur_level - 1) as usize);
        }
        match cursor.take_guard(cur_level as usize - 1) {
            GuardInPath::Unlocked => unreached(),
            GuardInPath::Read(mut rguard) => {
                let res = rguard.drop(Tracked(m));
                proof {
                    m = res.get();
                }
            },
            GuardInPath::Write(_) => unreached(),
            GuardInPath::ImplicitWrite(_) => unreached(),
        }
        assert(cursor.wf());
        assert(cursor.wf_with_lock_protocol_model(m)) by {
            assert(cursor.g_level@ > cursor.guard_level);
            assert forall|level: PagingLevel|
                #![trigger cursor.get_guard_level(level)]
                cursor.g_level@ <= level <= C::NR_LEVELS() implies {
                &&& cursor.get_guard_level(level) !is Unlocked
                &&& match cursor.get_guard_level(level) {
                    GuardInPath::Read(rguard) => m.path()[C::NR_LEVELS() - level] == rguard.nid(),
                    GuardInPath::Write(wguard) => m.path()[C::NR_LEVELS() - level] == wguard.nid(),
                    GuardInPath::ImplicitWrite(wguard) => true,
                    GuardInPath::Unlocked => true,
                }
            } by {
                assert(cursor.get_guard_level(level) =~= _cursor.get_guard_level(level)) by {
                    assert(cursor.path@ =~= _cursor.path@.update(
                        cur_level - 1,
                        GuardInPath::Unlocked,
                    ));
                    assert(cursor.path@[level - 1] =~= _cursor.path@[level - 1]) by {
                        axiom_seq_update_different(
                            _cursor.path@,
                            level - 1,
                            cur_level - 1,
                            GuardInPath::Unlocked,
                        );
                    };
                };
            };
        };
        cur_level += 1;
    }

    proof {
        let tracked token = cursor.inst.borrow().unlocking_end(m.cpu, m.token);
        m.token = token;
    }

    Tracked(m)
}

} // verus!
