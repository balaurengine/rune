//! Balaur fork: entries live in a vector in insertion order, and the hash
//! table holds only their positions, so a map or set iterates in the order a
//! script wrote it.

use rune_alloc::hash_map;

use core::hash::BuildHasher;
use core::iter;
use core::marker::PhantomData;
use core::mem;
use core::ptr;

use crate::alloc;
use crate::alloc::prelude::*;
use crate::alloc::Vec;

#[cfg(feature = "alloc")]
use crate::runtime::Hasher;
use crate::runtime::{ProtocolCaller, RawAnyGuard, Ref, Value, VmError, VmResult};

use crate::alloc::hashbrown::raw::RawTable;
use crate::alloc::hashbrown::ErrorOrInsertSlot;

pub(crate) struct Table<V> {
    entries: Vec<(Value, V)>,
    table: RawTable<usize>,
    state: hash_map::RandomState,
}

impl<V> Table<V> {
    #[inline(always)]
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
            table: RawTable::new(),
            state: hash_map::RandomState::new(),
        }
    }

    #[inline(always)]
    pub(crate) fn try_with_capacity(capacity: usize) -> alloc::Result<Self> {
        Ok(Self {
            entries: Vec::try_with_capacity(capacity)?,
            table: RawTable::try_with_capacity(capacity)?,
            state: hash_map::RandomState::new(),
        })
    }

    #[inline(always)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[inline(always)]
    pub(crate) fn capacity(&self) -> usize {
        self.table.capacity()
    }

    #[inline(always)]
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// A key already present keeps its place; a new one goes last.
    pub(crate) fn insert_with(
        &mut self,
        key: Value,
        value: V,
        caller: &mut dyn ProtocolCaller,
    ) -> VmResult<Option<V>> {
        let hash = vm_try!(hash(&self.state, &key, caller));

        let found = self.table.find_or_find_insert_slot(
            caller,
            hash,
            KeyEq::new(&key, &self.entries),
            StateHasher::new(&self.state, &self.entries),
        );

        match found {
            Ok(bucket) => {
                let at = unsafe { *bucket.as_ref() };
                VmResult::Ok(Some(mem::replace(&mut self.entries[at].1, value)))
            }
            Err(ErrorOrInsertSlot::InsertSlot(slot)) => {
                let at = self.entries.len();
                vm_try!(self.entries.try_push((key, value)));
                unsafe {
                    self.table.insert_in_slot(hash, slot, at);
                }
                VmResult::Ok(None)
            }
            Err(ErrorOrInsertSlot::Error(error)) => VmResult::err(error),
        }
    }

    pub(crate) fn get(
        &self,
        key: &Value,
        caller: &mut dyn ProtocolCaller,
    ) -> VmResult<Option<&(Value, V)>> {
        if self.entries.is_empty() {
            return VmResult::Ok(None);
        }

        let hash = vm_try!(hash(&self.state, key, caller));
        let at = vm_try!(self.table.get(caller, hash, KeyEq::new(key, &self.entries)));
        VmResult::Ok(at.map(|&at| &self.entries[at]))
    }

    /// The rest keep their order, so every later position moves down one.
    #[inline(always)]
    pub(crate) fn remove_with(
        &mut self,
        key: &Value,
        caller: &mut dyn ProtocolCaller,
    ) -> VmResult<Option<V>> {
        let hash = vm_try!(hash(&self.state, key, caller));

        let removed = match self
            .table
            .remove_entry(caller, hash, KeyEq::new(key, &self.entries))
        {
            Ok(removed) => removed,
            Err(error) => return VmResult::Err(error),
        };

        let Some(at) = removed else {
            return VmResult::Ok(None);
        };

        let (_, value) = self.entries.remove(at);

        // SAFETY: the buckets are read and written while the table is borrowed
        // mutably, and none are added or removed.
        unsafe {
            for bucket in self.table.iter() {
                let slot = bucket.as_mut();
                if *slot > at {
                    *slot -= 1;
                }
            }
        }

        VmResult::Ok(Some(value))
    }

    #[inline(always)]
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.table.clear()
    }

    pub(crate) fn iter(&self) -> Iter<'_, V> {
        Iter {
            iter: RawEntries::new(&self.entries),
            _marker: PhantomData,
        }
    }

    #[inline(always)]
    pub(crate) fn iter_ref(this: Ref<Self>) -> IterRef<V> {
        let (this, _guard) = Ref::into_raw(this);
        // SAFETY: Table will be alive and a reference to it held for as long as
        // `RawAnyGuard` is alive.
        let iter = unsafe { RawEntries::new(&this.as_ref().entries) };
        IterRef {
            iter,
            guard: _guard,
        }
    }

    /// # Safety
    ///
    /// The table must stay borrowed for as long as the entries are walked.
    #[inline(always)]
    pub(crate) unsafe fn iter_ref_raw(this: ptr::NonNull<Table<V>>) -> RawEntries<V> {
        RawEntries::new(&this.as_ref().entries)
    }

    #[inline(always)]
    pub(crate) fn keys_ref(this: Ref<Self>) -> KeysRef<V> {
        let (this, _guard) = Ref::into_raw(this);
        // SAFETY: Table will be alive and a reference to it held for as long as
        // `RawAnyGuard` is alive.
        let iter = unsafe { RawEntries::new(&this.as_ref().entries) };
        KeysRef {
            iter,
            guard: _guard,
        }
    }

    #[inline(always)]
    pub(crate) fn values_ref(this: Ref<Self>) -> ValuesRef<V> {
        let (this, _guard) = Ref::into_raw(this);
        // SAFETY: Table will be alive and a reference to it held for as long as
        // `RawAnyGuard` is alive.
        let iter = unsafe { RawEntries::new(&this.as_ref().entries) };
        ValuesRef {
            iter,
            guard: _guard,
        }
    }
}

impl<V> TryClone for Table<V>
where
    V: TryClone,
{
    fn try_clone(&self) -> alloc::Result<Self> {
        Ok(Self {
            entries: self.entries.try_clone()?,
            table: self.table.try_clone()?,
            state: self.state.clone(),
        })
    }
}

/// Entries walked by pointer, for iterators that hold the table's borrow
/// guard instead of a lifetime.
pub(crate) struct RawEntries<V> {
    at: *const (Value, V),
    left: usize,
}

impl<V> RawEntries<V> {
    fn new(entries: &[(Value, V)]) -> Self {
        Self {
            at: entries.as_ptr(),
            left: entries.len(),
        }
    }

    /// # Safety
    ///
    /// The table the entries came from must still be borrowed.
    pub(crate) unsafe fn next<'a>(&mut self) -> Option<&'a (Value, V)> {
        if self.left == 0 {
            return None;
        }
        let entry = &*self.at;
        self.at = self.at.add(1);
        self.left -= 1;
        Some(entry)
    }

    pub(crate) fn size_hint(&self) -> (usize, Option<usize>) {
        (self.left, Some(self.left))
    }

    pub(crate) fn len(&self) -> usize {
        self.left
    }
}

pub(crate) struct Iter<'a, V> {
    iter: RawEntries<V>,
    _marker: PhantomData<&'a V>,
}

impl<'a, V> iter::Iterator for Iter<'a, V> {
    type Item = &'a (Value, V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // SAFETY: the iterator borrows the table for `'a`.
        unsafe { self.iter.next() }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

pub(crate) struct IterRef<V> {
    iter: RawEntries<V>,
    #[allow(unused)]
    guard: RawAnyGuard,
}

impl<V> iter::Iterator for IterRef<V>
where
    V: Clone,
{
    type Item = (Value, V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // SAFETY: we're still holding onto the `RawAnyGuard` guard.
        unsafe { Some(self.iter.next()?.clone()) }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<V> iter::ExactSizeIterator for IterRef<V>
where
    V: Clone,
{
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

pub(crate) struct KeysRef<V> {
    iter: RawEntries<V>,
    #[allow(unused)]
    guard: RawAnyGuard,
}

impl<V> iter::Iterator for KeysRef<V> {
    type Item = Value;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // SAFETY: we're still holding onto the `RawAnyGuard` guard.
        unsafe { Some(self.iter.next()?.0.clone()) }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

pub(crate) struct ValuesRef<V> {
    iter: RawEntries<V>,
    #[allow(unused)]
    guard: RawAnyGuard,
}

impl<V> iter::Iterator for ValuesRef<V>
where
    V: Clone,
{
    type Item = V;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // SAFETY: we're still holding onto the `RawAnyGuard` guard.
        unsafe { Some(self.iter.next()?.1.clone()) }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

/// Convenience function to hash a value.
fn hash<S>(state: &S, value: &Value, caller: &mut dyn ProtocolCaller) -> VmResult<u64>
where
    S: BuildHasher<Hasher = hash_map::Hasher>,
{
    let mut hasher = Hasher::new_with(state);
    vm_try!(value.hash_with(&mut hasher, caller));
    VmResult::Ok(hasher.finish())
}

/// Hashes the key an index points at, when the table grows.
struct StateHasher<'a, V> {
    state: &'a hash_map::RandomState,
    entries: &'a [(Value, V)],
}

impl<'a, V> StateHasher<'a, V> {
    #[inline]
    fn new(state: &'a hash_map::RandomState, entries: &'a [(Value, V)]) -> Self {
        Self { state, entries }
    }
}

impl<V> alloc::hashbrown::HasherFn<dyn ProtocolCaller, usize, VmError> for StateHasher<'_, V> {
    #[inline]
    fn hash(&self, cx: &mut dyn ProtocolCaller, at: &usize) -> Result<u64, VmError> {
        hash(self.state, &self.entries[*at].0, cx).into_result()
    }
}

/// Compares the key being looked up with the key an index points at.
struct KeyEq<'a, V> {
    key: &'a Value,
    entries: &'a [(Value, V)],
}

impl<'a, V> KeyEq<'a, V> {
    #[inline]
    fn new(key: &'a Value, entries: &'a [(Value, V)]) -> Self {
        Self { key, entries }
    }
}

impl<V> alloc::hashbrown::EqFn<dyn ProtocolCaller, usize, VmError> for KeyEq<'_, V> {
    #[inline]
    fn eq(&self, cx: &mut dyn ProtocolCaller, at: &usize) -> Result<bool, VmError> {
        self.key.eq_with(&self.entries[*at].0, cx).into_result()
    }
}
