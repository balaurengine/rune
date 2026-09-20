//! Balaur fork: a map that iterates in insertion order, which is what an
//! object hands a script through `iter`, `keys` and `values`.
//!
//! Hash order was already the same on every machine, but it is not the order
//! a script wrote its keys in, and it would move if the hash ever changed.

use core::borrow::Borrow;
use core::hash::Hash;
use core::slice;

use crate::alloc::prelude::*;
use crate::alloc::{self, HashMap, Vec};

pub(crate) struct OrderedMap<K, V> {
    entries: Vec<(K, V)>,
    index: HashMap<K, usize>,
}

impl<K, V> Default for OrderedMap<K, V> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
        }
    }
}

impl<K, V> OrderedMap<K, V>
where
    K: Hash + Eq + TryClone,
{
    pub(crate) fn try_with_capacity(capacity: usize) -> alloc::Result<Self> {
        Ok(Self {
            entries: Vec::try_with_capacity(capacity)?,
            index: HashMap::try_with_capacity(capacity)?,
        })
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn get<Q>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let at = *self.index.get(k)?;
        Some(&self.entries[at].1)
    }

    pub(crate) fn get_mut<Q>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let at = *self.index.get(k)?;
        Some(&mut self.entries[at].1)
    }

    pub(crate) fn contains_key<Q>(&self, k: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        self.index.contains_key(k)
    }

    /// A key already present keeps its place; a new one goes last.
    pub(crate) fn try_insert(&mut self, k: K, v: V) -> alloc::Result<Option<V>> {
        if let Some(&at) = self.index.get(&k) {
            return Ok(Some(core::mem::replace(&mut self.entries[at].1, v)));
        }
        self.index.try_insert(k.try_clone()?, self.entries.len())?;
        self.entries.try_push((k, v))?;
        Ok(None)
    }

    /// The rest keep their order, so every later entry moves down one.
    pub(crate) fn remove<Q>(&mut self, k: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let at = self.index.remove(k)?;
        let (_, v) = self.entries.remove(at);
        for (key, _) in &self.entries[at..] {
            if let Some(slot) = self.index.get_mut(key) {
                *slot -= 1;
            }
        }
        Some(v)
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub(crate) fn entries(&self) -> &[(K, V)] {
        &self.entries
    }

    pub(crate) fn entries_mut(&mut self) -> &mut [(K, V)] {
        &mut self.entries
    }

    pub(crate) fn into_entries(self) -> Vec<(K, V)> {
        self.entries
    }

    pub(crate) fn iter(&self) -> slice::Iter<'_, (K, V)> {
        self.entries.iter()
    }
}

impl<K, V> TryClone for OrderedMap<K, V>
where
    K: TryClone,
    V: TryClone,
{
    fn try_clone(&self) -> alloc::Result<Self> {
        Ok(Self {
            entries: self.entries.try_clone()?,
            index: self.index.try_clone()?,
        })
    }
}

impl<K, V> core::fmt::Debug for OrderedMap<K, V>
where
    K: core::fmt::Debug,
    V: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_map()
            .entries(self.entries.iter().map(|(k, v)| (k, v)))
            .finish()
    }
}

/// Written as its entries, so the order survives a round trip.
impl<K, V> serde::Serialize for OrderedMap<K, V>
where
    K: serde::Serialize,
    V: serde::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.entries.len()))?;
        for (key, value) in self.entries.iter() {
            seq.serialize_element(&(key, value))?;
        }
        seq.end()
    }
}

impl<'de, K, V> serde::Deserialize<'de> for OrderedMap<K, V>
where
    K: serde::Deserialize<'de> + Hash + Eq + TryClone,
    V: serde::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(EntriesVisitor(core::marker::PhantomData))
    }
}

struct EntriesVisitor<K, V>(core::marker::PhantomData<(K, V)>);

impl<'de, K, V> serde::de::Visitor<'de> for EntriesVisitor<K, V>
where
    K: serde::Deserialize<'de> + Hash + Eq + TryClone,
    V: serde::Deserialize<'de>,
{
    type Value = OrderedMap<K, V>;

    fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("a sequence of key and value pairs")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        use serde::de::Error as _;
        let mut map = OrderedMap::try_with_capacity(seq.size_hint().unwrap_or_default())
            .map_err(A::Error::custom)?;
        while let Some((key, value)) = seq.next_element::<(K, V)>()? {
            map.try_insert(key, value).map_err(A::Error::custom)?;
        }
        Ok(map)
    }
}
