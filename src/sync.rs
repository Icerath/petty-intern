use {
    rustc_hash::FxBuildHasher,
    std::{
        borrow::Borrow,
        fmt,
        hash::{BuildHasher, Hash},
        sync::RwLock,
    },
};

pub struct Interner<T> {
    inner: RwLock<crate::Interner<T>>,
}

impl<T> Default for Interner<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Interner<T> {
    /// Creates an empty Interner.
    /// The current implementation does not allocate
    #[must_use]
    pub const fn new() -> Self {
        Self { inner: RwLock::new(crate::Interner::new()) }
    }

    /// Returns the number of entries in the interner
    #[expect(clippy::missing_panics_doc)]
    pub fn len(&self) -> usize {
        self.inner.read().unwrap().len()
    }

    /// Returns `true` if the interner contains no elements
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T: Hash + Eq> Interner<T> {
    /// Will return a reference to an equivalent value if it already exists
    #[expect(clippy::missing_panics_doc)]
    #[must_use]
    pub fn try_resolve<Q>(&self, value: &Q) -> Option<&T>
    where
        T: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        self.inner.read().unwrap().try_resolve(value).map(|cached| unsafe { longer(cached) })
    }

    /// Returns a reference to either the value provided, or an equivalent value that was already inserted
    #[expect(clippy::missing_panics_doc, clippy::readonly_write_lock)]
    pub fn intern(&self, value: T) -> &T {
        let hash = FxBuildHasher.hash_one(&value);

        let inner = self.inner.read().unwrap();
        if let Some(cached) = inner.try_resolve_with(&value, hash) {
            return unsafe { longer(cached) };
        }

        drop(inner);
        let inner = self.inner.write().unwrap();

        // try again in case another thread inserted a value in between the drop(_) and the .writer().
        // It would be nice to avoid the last 2 lookups if we could 'upgrade' the read guard and ask if there were any writes in between.
        if let Some(cached) = inner.try_resolve_with(&value, hash) {
            return unsafe { longer(cached) };
        }

        unsafe { longer(inner.insert(hash, value)) }
    }

    #[expect(clippy::missing_panics_doc, clippy::readonly_write_lock)]
    /// Inserts the value into the interner without checking if the value already exists
    pub fn intern_new(&self, value: T) -> &T {
        let hash = FxBuildHasher.hash_one(&value);
        let inner = self.inner.write().unwrap();
        unsafe { longer(inner.insert(hash, value)) }
    }
}

impl<T: fmt::Debug> fmt::Debug for Interner<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Ok(inner) = self.inner.try_read() else {
            return f.debug_set().finish_non_exhaustive();
        };
        inner.fmt(f)
    }
}

unsafe fn longer<'b, T>(short: &T) -> &'b T {
    unsafe { std::mem::transmute(short) }
}

unsafe impl<T: Send> Send for Interner<T> {}
unsafe impl<T: Sync> Sync for Interner<T> {}

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
