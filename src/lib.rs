use std::{
    fmt,
    hash::Hash,
    sync::{Mutex, OnceLock},
};

use parking_lot::{RwLock, RwLockUpgradableReadGuard};

use rustc_hash::{FxBuildHasher, FxHashSet};
use typed_arena::Arena;

pub struct Interner<T: 'static> {
    storage: RwLock<FxHashSet<&'static T>>,
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
            storage: RwLock::new(FxHashSet::with_hasher(FxBuildHasher)),
            arena: OnceLock::new(),
        }
    }
}

impl<T: Hash + Eq> Interner<T> {
    pub fn intern(&self, value: T) -> &T {
        if let Some(cached) = self.storage.read().get(&value) {
            return cached;
        }
        let storage = self.storage.upgradable_read();

        let arena = self.arena.get_or_init(Mutex::default).lock().unwrap();
        let cached = arena.alloc(value);
        let cached: &'static mut T = unsafe { std::mem::transmute(cached) };
        drop(arena);
        let mut storage = RwLockUpgradableReadGuard::upgrade(storage);
        storage.insert(cached);
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
