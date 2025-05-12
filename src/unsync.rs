use {
    bumpalo::Bump,
    hashbrown::HashTable,
    rustc_hash::FxBuildHasher,
    std::{
        borrow::Borrow,
        cell::{OnceCell, UnsafeCell},
        fmt,
        hash::{BuildHasher, Hash},
        marker::PhantomData,
        ptr::NonNull,
    },
};

pub struct Interner<T> {
    // an interner must be covariant in `T`
    __marker: PhantomData<T>,
    // UnsafeCell for interior mutability, the NonNull<u8> is a reference into the arena.
    // It uses u8 instead of T to avoid making T invariant
    set: UnsafeCell<HashTable<NonNull<u8>>>,
    arena: OnceCell<Bump>,
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
        Self {
            __marker: PhantomData,
            set: UnsafeCell::new(HashTable::new()),
            arena: OnceCell::new(),
        }
    }

    /// Returns the number of entries in the interner
    pub fn len(&self) -> usize {
        self.set().len()
    }

    /// Returns `true` if the interner contains no elements
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // Inserts the value into the interner's arena without checking if the value already exists.
    // Future calls to intern will not find the same value, use `intern_new` if you want that behaviour.
    pub fn insert_arena(&self, value: T) -> &mut T {
        self.arena.get_or_init(Bump::new).alloc(value)
    }

    fn set(&self) -> &HashTable<NonNull<u8>> {
        // Safety: mutable access is entirely contained without the Interners methods.
        unsafe { self.set.get().as_ref().unwrap() }
    }

    #[expect(clippy::mut_from_ref)]
    fn set_mut(&self) -> &mut HashTable<NonNull<u8>> {
        // Safety: mutable access is entirely contained without the Interners methods.
        unsafe { self.set.get().as_mut().unwrap() }
    }
}

impl<T: Hash + Eq> Interner<T> {
    pub(crate) fn try_resolve_with<Q>(&self, value: &Q, hash: u64) -> Option<&T>
    where
        T: Borrow<Q>,
        Q: ?Sized + Eq,
    {
        self.set()
            .find(hash, |cached| T::borrow(unsafe { cached.cast().as_ref() }) == value)
            .map(|ptr| unsafe { ptr.cast().as_ref() })
    }

    pub(crate) fn insert(&self, hash: u64, value: T) -> &T {
        let arena = self.arena.get_or_init(Bump::new);

        let cached = NonNull::from(arena.alloc(value)).cast();
        self.set_mut().insert_unique(hash, cached, |t| FxBuildHasher.hash_one(t));
        unsafe { cached.cast().as_ref() }
    }

    /// Will return a reference to an equivalent value if it already exists
    #[must_use]
    pub fn try_resolve<Q>(&self, value: &Q) -> Option<&T>
    where
        T: Borrow<Q>,
        Q: ?Sized + Hash + Eq,
    {
        let hash = FxBuildHasher.hash_one(value);
        self.try_resolve_with(value, hash)
    }

    /// Returns a reference to either the value provided, or an equivalent value that was already inserted
    pub fn intern(&self, value: T) -> &T {
        let hash = FxBuildHasher.hash_one(&value);

        if let Some(cached) = self.try_resolve_with(&value, hash) {
            return cached;
        }

        self.insert(hash, value)
    }

    /// Inserts the value into the interner without checking if the value already exists
    pub fn intern_new(&self, value: T) -> &T {
        let hash = FxBuildHasher.hash_one(&value);
        self.insert(hash, value)
    }
}

impl<T: fmt::Debug> fmt::Debug for Interner<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.set().fmt(f)
    }
}

unsafe impl<T> Send for Interner<T> where T: Send {}
