use std::{
    fmt,
    hash::Hash,
    sync::{Mutex, OnceLock},
};

use hashbrown::HashTable;
use parking_lot::{RwLock, RwLockUpgradableReadGuard};
use std::hash::BuildHasher;

use rustc_hash::FxBuildHasher;
use typed_arena::Arena;

pub struct Interner<T: 'static> {
    storage: RwLock<HashTable<&'static T>>,
    arena: OnceLock<Mutex<Arena<T>>>,
}

impl<T: fmt::Debug> fmt::Debug for Interner<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug_map = f.debug_set();
        let Some(inner) = self.storage.try_read() else {
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
    pub const fn new() -> Self {
        Self {
            storage: RwLock::new(HashTable::new()),
            arena: OnceLock::new(),
        }
    }
}

impl<T: Hash + Eq> Interner<T> {
    pub fn intern(&self, value: T) -> &T {
        let hash = FxBuildHasher.hash_one(&value);

        let storage = self.storage.upgradable_read();
        if let Some(cached) = { storage.find(hash, |cached| *cached == &value).copied() } {
            return cached;
        }

        let arena = self.arena.get_or_init(Mutex::default).lock().unwrap();
        let cached = arena.alloc(value);
        let cached: &'static mut T = unsafe { std::mem::transmute(cached) };
        drop(arena);
        let mut storage = RwLockUpgradableReadGuard::upgrade(storage);
        storage.insert_unique(hash, cached, |t| FxBuildHasher.hash_one(t));
        cached
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn addr() {
        static INTERNER: Interner<i32> = Interner::new();

        let a1: *const _ = INTERNER.intern(1);
        let b1: *const _ = INTERNER.intern(1);
        INTERNER.intern(2);

        assert_eq!(a1.addr(), b1.addr());
    }
}
