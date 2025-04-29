use std::{
    borrow::Borrow,
    fmt,
    hash::Hash,
    ptr::NonNull,
    sync::{Mutex, OnceLock},
};

use hashbrown::HashTable;
use parking_lot::{RwLock, RwLockUpgradableReadGuard};
use std::hash::BuildHasher;

use rustc_hash::FxBuildHasher;
use typed_arena::Arena;

pub struct Interner<T> {
    set: RwLock<HashTable<NonNull<T>>>,
    arena: OnceLock<Mutex<Arena<T>>>,
}

impl<T: fmt::Debug> fmt::Debug for Interner<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug_map = f.debug_set();
        let Some(inner) = self.set.try_read() else {
            return debug_map.finish_non_exhaustive();
        };
        debug_map.entries(&*inner).finish()
    }
}

impl<T> Default for Interner<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Interner<T> {
    #[must_use]
    pub const fn new() -> Self {
        Self { set: RwLock::new(HashTable::new()), arena: OnceLock::new() }
    }
}

impl<T: Hash + Eq> Interner<T> {
    #[must_use]
    pub fn try_resolve<Q>(&self, value: &Q) -> Option<&T>
    where
        T: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let hash = FxBuildHasher.hash_one(value);

        unsafe {
            self.set
                .read()
                .find(hash, |cached| T::borrow(cached.as_ref()) == value)
                .map(|ptr| ptr.as_ref())
        }
    }

    #[expect(clippy::missing_panics_doc)]
    pub fn intern(&self, value: T) -> &T {
        let hash = FxBuildHasher.hash_one(&value);

        let set = self.set.upgradable_read();
        unsafe {
            if let Some(cached) = set.find(hash, |cached| cached.as_ref() == &value) {
                return cached.as_ref();
            }
        }

        let arena = self.arena.get_or_init(Mutex::default).lock().unwrap();
        let cached = NonNull::from(arena.alloc(value));
        drop(arena);
        let mut set = RwLockUpgradableReadGuard::upgrade(set);
        set.insert_unique(hash, cached, |t| FxBuildHasher.hash_one(t));
        unsafe { cached.as_ref() }
    }
}

// FIXME: is might be overly restrictive?
unsafe impl<T: Send + Sync> Send for Interner<T> {}
unsafe impl<T: Send + Sync> Sync for Interner<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn addr() {
        static INTERNER: Interner<i32> = Interner::new();

        let a1: *const _ = INTERNER.intern(1);
        let b1: *const _ = INTERNER.intern(1);
        INTERNER.intern(2);

        assert!(INTERNER.try_resolve(&1) == Some(&1));

        assert_eq!(a1.addr(), b1.addr());
    }
}
