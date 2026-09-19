use core::borrow;
use core::cmp;
use core::fmt;
use core::hash;
use core::iter;

use crate as rune;
use core::slice;

use crate::alloc::prelude::*;
use crate::alloc::{self, String};
use crate::runtime::ordered::OrderedMap;
use crate::runtime::{
    FromValue, ProtocolCaller, RawAnyGuard, Ref, ToValue, Value, VmError, VmResult,
};
use crate::Any;

/// Balaur fork: every iterator below walks the entries in insertion order.
pub type IntoIter = iter::Map<alloc::vec::IntoIter<(String, Value)>, fn((String, Value)) -> (String, Value)>;

/// A mutable iterator over the entries of a `Object`, in insertion order.
pub type IterMut<'a> = iter::Map<slice::IterMut<'a, (String, Value)>, fn(&'a mut (String, Value)) -> (&'a String, &'a mut Value)>;

/// An iterator over the entries of a `Object`, in insertion order.
pub type Iter<'a> = iter::Map<slice::Iter<'a, (String, Value)>, fn(&'a (String, Value)) -> (&'a String, &'a Value)>;

/// An iterator over the keys of an `Object`, in insertion order.
pub type Keys<'a> = iter::Map<slice::Iter<'a, (String, Value)>, fn(&'a (String, Value)) -> &'a String>;

/// An iterator over the values of an `Object`, in insertion order.
pub type Values<'a> = iter::Map<slice::Iter<'a, (String, Value)>, fn(&'a (String, Value)) -> &'a Value>;

/// Struct representing a dynamic anonymous object.
///
/// # Rust Examples
///
/// ```rust
/// use rune::alloc::String;
///
/// let mut object = rune::runtime::Object::new();
/// assert!(object.is_empty());
///
/// object.insert_value(String::try_from("foo")?, 42).into_result()?;
/// object.insert_value(String::try_from("bar")?, true).into_result()?;
/// assert_eq!(2, object.len());
///
/// assert_eq!(Some(42), object.get_value("foo").into_result()?);
/// assert_eq!(Some(true), object.get_value("bar").into_result()?);
/// assert_eq!(None::<bool>, object.get_value("baz").into_result()?);
/// # Ok::<_, rune::support::Error>(())
/// ```
#[derive(Any, Default)]
#[repr(transparent)]
#[rune(item = ::std::object)]
pub struct Object {
    inner: OrderedMap<String, Value>,
}

impl Object {
    /// Construct a new object.
    ///
    /// # Examples
    ///
    /// ```rune
    /// let object = Object::new();
    /// object.insert("Hello", "World");
    /// ```
    #[inline]
    #[rune::function(keep, path = Self::new)]
    pub fn new() -> Self {
        Self {
            inner: OrderedMap::default(),
        }
    }

    /// Construct a new object with the given capacity.
    ///
    /// # Examples
    ///
    /// ```rune
    /// let object = Object::with_capacity(16);
    /// object.insert("Hello", "World");
    /// ```
    #[inline]
    #[rune::function(path = Self::with_capacity)]
    pub(crate) fn rune_with_capacity(capacity: usize) -> VmResult<Self> {
        VmResult::Ok(vm_try!(Self::with_capacity(capacity)))
    }

    /// Construct a new object with the given capacity.
    pub fn with_capacity(capacity: usize) -> alloc::Result<Self> {
        // BTreeMap doesn't support setting capacity on creation but we keep
        // this here in case we want to switch store later.
        Ok(Self {
            inner: OrderedMap::try_with_capacity(capacity)?,
        })
    }

    /// Returns the number of elements in the object.
    ///
    /// # Examples
    ///
    /// ```rune
    /// let object = Object::with_capacity(16);
    /// object.insert("Hello", "World");
    /// assert_eq!(object.len(), 1);
    /// ```
    #[inline]
    #[rune::function(keep)]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the object is empty.
    ///
    /// # Examples
    ///
    /// ```rune
    /// let object = Object::with_capacity(16);
    /// assert!(object.is_empty());
    /// object.insert("Hello", "World");
    /// assert!(!object.is_empty());
    /// ```
    #[inline]
    #[rune::function(keep)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns a reference to the value corresponding to the key.
    #[inline]
    pub fn get<Q>(&self, k: &Q) -> Option<&Value>
    where
        String: borrow::Borrow<Q>,
        Q: ?Sized + hash::Hash + cmp::Eq + cmp::Ord,
    {
        self.inner.get(k)
    }

    /// Get the given value at the given index.
    pub fn get_value<Q, T>(&self, k: &Q) -> VmResult<Option<T>>
    where
        String: borrow::Borrow<Q>,
        Q: ?Sized + hash::Hash + cmp::Eq + cmp::Ord,
        T: FromValue,
    {
        let value = match self.inner.get(k) {
            Some(value) => value.clone(),
            None => return VmResult::Ok(None),
        };

        VmResult::Ok(Some(vm_try!(T::from_value(value))))
    }

    /// Returns a mutable reference to the value corresponding to the key.
    #[inline]
    pub fn get_mut<Q>(&mut self, k: &Q) -> Option<&mut Value>
    where
        String: borrow::Borrow<Q>,
        Q: ?Sized + hash::Hash + cmp::Eq + cmp::Ord,
    {
        self.inner.get_mut(k)
    }

    /// Returns `true` if the map contains a value for the specified key.
    #[inline]
    pub fn contains_key<Q>(&self, k: &Q) -> bool
    where
        String: borrow::Borrow<Q>,
        Q: ?Sized + hash::Hash + cmp::Eq + cmp::Ord,
    {
        self.inner.contains_key(k)
    }

    /// Removes a key from the map, returning the value at the key if the key
    /// was previously in the map.
    #[inline]
    pub fn remove<Q>(&mut self, k: &Q) -> Option<Value>
    where
        String: borrow::Borrow<Q>,
        Q: ?Sized + hash::Hash + cmp::Eq + cmp::Ord,
    {
        self.inner.remove(k)
    }

    /// Inserts a key-value pair into the dynamic object, converting it as
    /// necessary through the [`ToValue`] trait.
    #[inline]
    pub fn insert_value<T>(&mut self, k: String, v: T) -> VmResult<()>
    where
        T: ToValue,
    {
        vm_try!(self.inner.try_insert(k, vm_try!(v.to_value())));
        VmResult::Ok(())
    }

    /// Inserts a key-value pair into the map.
    ///
    /// If the map did not have this key present, `None` is returned.
    ///
    /// # Examples
    ///
    /// ```rune
    /// let map = #{};
    /// assert_eq!(map.insert("a", 1), None);
    /// assert_eq!(map.is_empty(), false);
    ///
    /// map.insert("b", 2);
    /// assert_eq!(map.insert("b", 3), Some(2));
    /// assert_eq!(map["b"], 3);
    /// ```
    #[inline]
    #[rune::function(path = Self::insert)]
    pub(crate) fn rune_insert(&mut self, k: String, v: Value) -> VmResult<Option<Value>> {
        VmResult::Ok(vm_try!(self.inner.try_insert(k, v)))
    }

    /// Inserts a key-value pair into the map.
    ///
    /// If the map did not have this key present, `None` is returned.
    #[inline]
    pub fn insert(&mut self, k: String, v: Value) -> alloc::Result<Option<Value>> {
        self.inner.try_insert(k, v)
    }

    /// Clears the object, removing all key-value pairs. Keeps the allocated
    /// memory for reuse.
    #[inline]
    #[rune::function(keep)]
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// An iterator visiting all key-value pairs in arbitrary order.
    /// The iterator element type is `(&'a String, &'a Value)`.
    pub fn iter(&self) -> Iter<'_> {
        self.inner.entries().iter().map(|(k, v)| (k, v))
    }

    /// An iterator visiting all keys in arbitrary order.
    /// The iterator element type is `&'a String`.
    pub fn keys(&self) -> Keys<'_> {
        self.inner.entries().iter().map(|(k, _)| k)
    }

    /// An iterator visiting all values in arbitrary order.
    /// The iterator element type is `&'a Value`.
    pub fn values(&self) -> Values<'_> {
        self.inner.entries().iter().map(|(_, v)| v)
    }

    /// An iterator visiting all key-value pairs in arbitrary order,
    /// with mutable references to the values.
    ///
    /// The iterator element type is `(&'a String, &'a mut Value)`.
    pub fn iter_mut(&mut self) -> IterMut<'_> {
        self.inner.entries_mut().iter_mut().map(|(k, v)| (&*k, v))
    }

    /// An iterator visiting all keys and values in arbitrary order.
    ///
    /// # Examples
    ///
    /// ```rune
    /// let object = #{a: 1, b: 2, c: 3};
    /// let vec = [];
    ///
    /// for key in object.iter() {
    ///     vec.push(key);
    /// }
    ///
    /// vec.sort_by(|a, b| a.0.cmp(b.0));
    /// assert_eq!(vec, [("a", 1), ("b", 2), ("c", 3)]);
    /// ```
    #[rune::function(keep, path = Self::iter)]
    pub fn rune_iter(this: Ref<Self>) -> RuneIter {
        // SAFETY: we're holding onto the related reference guard, and making
        // sure that it's dropped after the iterator.
        let iter = RawEntries::new(this.inner.entries());
        let (_, guard) = Ref::into_raw(this);
        RuneIter { iter, guard }
    }

    /// An iterator visiting all keys in arbitrary order.
    ///
    /// # Examples
    ///
    /// ```rune
    /// let object = #{a: 1, b: 2, c: 3};
    /// let vec = [];
    ///
    /// for key in object.keys() {
    ///     vec.push(key);
    /// }
    ///
    /// vec.sort_by(|a, b| a.cmp(b));
    /// assert_eq!(vec, ["a", "b", "c"]);
    /// ```
    #[rune::function(keep, path = Self::keys)]
    pub fn rune_keys(this: Ref<Self>) -> RuneIterKeys {
        // SAFETY: we're holding onto the related reference guard, and making
        // sure that it's dropped after the iterator.
        let iter = RawEntries::new(this.inner.entries());
        let (_, guard) = Ref::into_raw(this);
        RuneIterKeys { iter, guard }
    }

    /// An iterator visiting all values in arbitrary order.
    ///
    /// # Examples
    ///
    /// ```rune
    /// let object = #{a: 1, b: 2, c: 3};
    /// let vec = [];
    ///
    /// for key in object.values() {
    ///     vec.push(key);
    /// }
    ///
    /// vec.sort_by(|a, b| a.cmp(b));
    /// assert_eq!(vec, [1, 2, 3]);
    /// ```
    #[rune::function(keep, path = Self::values)]
    pub fn rune_values(this: Ref<Self>) -> RuneValues {
        // SAFETY: we're holding onto the related reference guard, and making
        // sure that it's dropped after the iterator.
        let iter = RawEntries::new(this.inner.entries());
        let (_, guard) = Ref::into_raw(this);
        RuneValues { iter, guard }
    }

    pub(crate) fn partial_eq_with(
        a: &Self,
        b: &Self,
        caller: &mut dyn ProtocolCaller,
    ) -> VmResult<bool> {
        if a.len() != b.len() {
            return VmResult::Ok(false);
        }

        for (k1, v1) in a.iter() {
            let Some(v2) = b.get(k1) else {
                return VmResult::Ok(false);
            };

            if !vm_try!(Value::partial_eq_with(v1, v2, caller)) {
                return VmResult::Ok(false);
            }
        }

        VmResult::Ok(true)
    }

    pub(crate) fn eq_with(
        a: &Self,
        b: &Self,
        eq: fn(&Value, &Value, &mut dyn ProtocolCaller) -> VmResult<bool>,
        caller: &mut dyn ProtocolCaller,
    ) -> VmResult<bool> {
        if a.inner.len() != b.inner.len() {
            return VmResult::Ok(false);
        }

        for (key, a) in a.inner.iter() {
            let Some(b) = b.inner.get(key) else {
                return VmResult::Ok(false);
            };

            if !vm_try!(eq(a, b, caller)) {
                return VmResult::Ok(false);
            }
        }

        VmResult::Ok(true)
    }
}

impl TryClone for Object {
    fn try_clone(&self) -> alloc::Result<Self> {
        Ok(Self {
            inner: self.inner.try_clone()?,
        })
    }
}

impl<'a> IntoIterator for &'a Object {
    type Item = (&'a String, &'a Value);
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut Object {
    type Item = (&'a String, &'a mut Value);
    type IntoIter = IterMut<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl IntoIterator for Object {
    type Item = (String, Value);
    type IntoIter = IntoIter;

    /// Creates a consuming iterator, that is, one that moves each key-value
    /// pair out of the object in arbitrary order. The object cannot be used
    /// after calling this.
    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_entries().into_iter().map(|e| e)
    }
}

impl fmt::Debug for Object {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

#[derive(Any)]
#[rune(item = ::std::object, name = Iter)]
pub struct RuneIter {
    iter: RawEntries,
    #[allow(unused)]
    guard: RawAnyGuard,
}

impl RuneIter {
    #[rune::function(instance, keep, protocol = NEXT)]
    pub fn next(&mut self) -> VmResult<Option<(String, Value)>> {
        unsafe {
            let Some((key, value)) = self.iter.next() else {
                return VmResult::Ok(None);
            };
            let key = vm_try!(key.try_clone());
            VmResult::Ok(Some((key, value.clone())))
        }
    }

    #[rune::function(instance, keep, protocol = SIZE_HINT)]
    pub fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    #[rune::function(instance, keep, protocol = LEN)]
    pub fn len(&self) -> usize {
        self.iter.len()
    }
}

impl iter::Iterator for RuneIter {
    type Item = Result<(String, Value), VmError>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match RuneIter::next(self) {
            VmResult::Ok(Some(value)) => Some(Ok(value)),
            VmResult::Ok(None) => None,
            VmResult::Err(err) => Some(Err(err)),
        }
    }
}

#[derive(Any)]
#[rune(item = ::std::object, name = Keys)]
pub struct RuneIterKeys {
    iter: RawEntries,
    #[allow(unused)]
    guard: RawAnyGuard,
}

impl RuneIterKeys {
    #[rune::function(instance, keep, protocol = NEXT)]
    pub fn next(&mut self) -> VmResult<Option<String>> {
        unsafe {
            let Some((key, _)) = self.iter.next() else {
                return VmResult::Ok(None);
            };
            let key = vm_try!(key.try_clone());
            VmResult::Ok(Some(key))
        }
    }

    #[rune::function(instance, keep, protocol = SIZE_HINT)]
    pub fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    #[rune::function(instance, keep, protocol = LEN)]
    pub fn len(&self) -> usize {
        self.iter.len()
    }
}

impl iter::Iterator for RuneIterKeys {
    type Item = Result<String, VmError>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match RuneIterKeys::next(self) {
            VmResult::Ok(Some(value)) => Some(Ok(value)),
            VmResult::Ok(None) => None,
            VmResult::Err(err) => Some(Err(err)),
        }
    }
}

#[derive(Any)]
#[rune(item = ::std::object, name = Values)]
pub struct RuneValues {
    iter: RawEntries,
    #[allow(unused)]
    guard: RawAnyGuard,
}

impl RuneValues {
    #[rune::function(instance, keep, protocol = NEXT)]
    pub fn next(&mut self) -> VmResult<Option<Value>> {
        unsafe {
            let Some((_, value)) = self.iter.next() else {
                return VmResult::Ok(None);
            };
            VmResult::Ok(Some(value.clone()))
        }
    }

    #[rune::function(instance, keep, protocol = SIZE_HINT)]
    pub fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    #[rune::function(instance, keep, protocol = LEN)]
    pub fn len(&self) -> usize {
        self.iter.len()
    }
}

impl iter::Iterator for RuneValues {
    type Item = Result<Value, VmError>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match RuneValues::next(self) {
            VmResult::Ok(Some(value)) => Some(Ok(value)),
            VmResult::Ok(None) => None,
            VmResult::Err(err) => Some(Err(err)),
        }
    }
}

/// The entries a script iterator walks. The object stays borrowed through the
/// iterator's guard, so the slice cannot move under it.
struct RawEntries {
    at: *const (String, Value),
    left: usize,
}

impl RawEntries {
    fn new(entries: &[(String, Value)]) -> Self {
        Self {
            at: entries.as_ptr(),
            left: entries.len(),
        }
    }

    /// # Safety
    ///
    /// The object the entries came from must still be borrowed.
    unsafe fn next<'a>(&mut self) -> Option<&'a (String, Value)> {
        if self.left == 0 {
            return None;
        }
        let entry = &*self.at;
        self.at = self.at.add(1);
        self.left -= 1;
        Some(entry)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.left, Some(self.left))
    }

    fn len(&self) -> usize {
        self.left
    }
}
