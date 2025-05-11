use {
    bumpalo::Bump,
    rustc_hash::FxBuildHasher,
    std::{
        borrow::Borrow,
        fmt,
        hash::{BuildHasher, Hash},
        ptr::NonNull,
        sync::RwLock,
    },
};

type Arena = Bump;

pub struct Interner<T> {
    inner: RwLock<crate::Interner<T>>,
}

impl<T: fmt::Debug> fmt::Debug for Interner<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Ok(inner) = self.inner.try_read() else {
            return f.debug_set().finish_non_exhaustive();
        };
        inner.fmt(f)
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
        Self { inner: RwLock::new(crate::Interner::new()) }
    }
}

impl<T: Hash + Eq> Interner<T> {
    #[expect(clippy::missing_panics_doc)]
    #[must_use]
    pub fn try_resolve<Q>(&self, value: &Q) -> Option<&T>
    where
        T: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        self.inner.read().unwrap().try_resolve(value).map(|cached| unsafe { longer(cached) })
    }

    #[expect(clippy::missing_panics_doc)]
    pub fn intern(&self, value: T) -> &T {
        let hash = FxBuildHasher.hash_one(&value);

        let inner = self.inner.read().unwrap();
        if let Some(cached) = inner.try_resolve_with(&value, hash) {
            return unsafe { longer(cached) };
        }

        drop(inner);
        let mut inner = self.inner.write().unwrap();

        if let Some(cached) = inner.try_resolve_with(&value, hash) {
            return unsafe { longer(cached) };
        }

        let arena = inner.arena.get_or_init(Arena::default);

        let cached = NonNull::from(arena.alloc(value)).cast();
        inner.set.get_mut().insert_unique(hash, cached, |t| FxBuildHasher.hash_one(t));
        unsafe { cached.cast().as_ref() }
    }
}

unsafe fn longer<'b, T>(short: &T) -> &'b T {
    unsafe { std::mem::transmute(short) }
}

// FIXME: this might be overly restrictive?
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
    #[test]
    fn recursive() {
        #[derive(Debug, PartialEq, Eq, Hash)]
        enum Type<'tcx> {
            Int,
            Array(&'tcx Type<'tcx>),
        }

        let interner = Interner::new();
        let int = interner.intern(Type::Int);
        let array = interner.intern(Type::Array(int));
        println!("{array:?}");
    }
}
