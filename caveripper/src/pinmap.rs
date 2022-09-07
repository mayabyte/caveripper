use std::{collections::HashMap, pin::Pin, hash::Hash, cell::UnsafeCell, sync::{RwLock, RwLockReadGuard}, borrow::Borrow};

use rayon::prelude::IntoParallelIterator;

/// Thread-safe, append-only HashMap that can provide immutable references to its 
/// entries that don't block further additions to the map.
#[derive(Debug)]
pub struct PinMap<K: Eq + Hash, V> {
    inner: UnsafeCell<HashMap<K, Pin<Box<V>>>>,
    lock: RwLock<()>,
}

// Access to the inner HashMap is synchronized via the lock, so this should be safe.
unsafe impl<K: Eq + Hash + Sync, V: Sync> Sync for PinMap<K, V> {}

impl<K: Eq + Hash, V> PinMap<K, V> {
    pub fn new() -> Self {
        Self { 
            inner: UnsafeCell::new(HashMap::new()),
            lock: RwLock::new(()),
        }
    }

    /// Inserts a value into the map. Returns the value in the Err variant of the 
    /// return value if the key is already present.
    /// This method will block if there are other ongoing reads or writes to the map.
    pub fn insert(&self, key: K, value: V) -> Result<(), V> {
        if unsafe{
            // SAFETY: we ensure there are no ongoing mutations via the RwLock guard.
            let _lock = self.lock.read().expect("PinMap lock poisoned");
            (*self.inner.get()).contains_key(&key)
        } {
            Err(value)
        }
        else {
            unsafe {
                // SAFETY: we ensure there are no other ongoing reads via the RwLock guard.
                // The map's contents are individually pinned, so any reallocation the map
                // does will not invalidate immutable references to the entries.
                let _lock = self.lock.write().expect("PinMap lock poisoned");
                self.inner.get().as_mut().unwrap().insert(key, Box::pin(value));
            }
            Ok(())
        }
    }

    /// Retrieves a reference to an entry in the map.
    /// The returned reference can be held for as long as the PinMap lives, even
    /// if there are insertions to the map afterwards.
    pub fn get<'a, Q: Eq + Hash>(&'a self, key: &Q) -> Option<&'a V> where K: Borrow<Q> {
        unsafe {
            // SAFETY: we ensure there are no ongoing reads or writes via the RwLock guard.
            let _lock = self.lock.read().expect("PinMap lock poisoned");
            (*self.inner.get()).get(key).map(|v| v.as_ref().get_ref())
        }
    }

    /// An iterator with the same behavior as the underlying HashMap's `.iter()`:
    /// visits all key-value pairs in arbitrary order.
    /// As long as this iterator object is alive, it holds a read lock to its parent
    /// PinMap, preventing any insertions from occurring.
    pub fn iter(&self) -> PinMapIterator<'_, K, Pin<Box<V>>> {
        unsafe {
            // SAFETY: since PinMapIterator holds a read guard, no new writes will
            // be able to occur while the iterator is alive.
            PinMapIterator { 
                inner: (*self.inner.get()).iter(), 
                _guard: self.lock.read().expect("PinMap lock poisoned"),
            }
        }
    }

    /// Returns the number of elements in the map.
    pub fn len(&self) -> usize {
        unsafe { (*self.inner.get()).len() }
    }
}

impl<K: Eq + Hash + Clone, V: Clone> Clone for PinMap<K, V> {
    fn clone(&self) -> Self {
        unsafe {
            // SAFETY: we ensure there are no ongoing reads or writes via the RwLock guard.
            let _lock = self.lock.read().expect("PinMap lock poisoned");
            PinMap { 
                inner: UnsafeCell::new((*self.inner.get()).clone()), 
                lock: RwLock::new(()), 
            }
        }
    }
}

impl <K: Eq + Hash, V> Default for PinMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PinMapIterator<'a, K: Eq + Hash, V> {
    pub(self) inner: std::collections::hash_map::Iter<'a, K, V>,
    pub(self) _guard: RwLockReadGuard<'a, ()>
}

impl<'a, K: Eq + Hash, V> Iterator for PinMapIterator<'a, K, V> {
    type Item = <std::collections::hash_map::Iter<'a, K, V> as Iterator>::Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<'a, K: Eq + Hash, V> IntoIterator for &'a PinMap<K, V> {
    type IntoIter = PinMapIterator<'a, K, Pin<Box<V>>>;
    type Item = (&'a K, &'a Pin<Box<V>>);
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

// Owned iterator implementations follow (no safety guarantees to uphold)

impl<K: Eq + Hash, V> IntoIterator for PinMap<K, V> {
    type IntoIter = <HashMap<K, Pin<Box<V>>> as IntoIterator>::IntoIter;
    type Item = <HashMap<K, Pin<Box<V>>> as IntoIterator>::Item;
    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_inner().into_iter()
    }
}

impl<K: Eq + Hash + Sync, V: Sync> IntoParallelIterator for PinMap<K, V> 
where HashMap<K, Pin<Box<V>>>: IntoParallelIterator
{
    type Iter = <HashMap<K, Pin<Box<V>>> as IntoParallelIterator>::Iter;
    type Item = <HashMap<K, Pin<Box<V>>> as IntoParallelIterator>::Item;
    fn into_par_iter(self) -> Self::Iter {
        self.inner.into_inner().into_par_iter()
    }
}
