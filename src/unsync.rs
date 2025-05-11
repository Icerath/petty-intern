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

type Arena = Bump;

pub struct Interner<T> {
    __marker: PhantomData<T>,
    pub(crate) set: UnsafeCell<HashTable<NonNull<u8>>>,
    pub(crate) arena: OnceCell<Arena>,
}

impl<T> Default for Interner<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Interner<T> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            __marker: PhantomData,
            set: UnsafeCell::new(HashTable::new()),
            arena: OnceCell::new(),
        }
    }
    pub(crate) fn set(&self) -> &HashTable<NonNull<u8>> {
        unsafe { self.set.get().as_ref().unwrap() }
    }
    #[expect(clippy::mut_from_ref)]
    pub(crate) fn set_mut(&self) -> &mut HashTable<NonNull<u8>> {
        unsafe { self.set.get().as_mut().unwrap() }
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
        self.try_resolve_with(value, hash)
    }

    pub(crate) fn try_resolve_with<Q>(&self, value: &Q, hash: u64) -> Option<&T>
    where
        T: Borrow<Q>,
        Q: ?Sized + Eq,
    {
        self.set()
            .find(hash, |cached| T::borrow(unsafe { cached.cast().as_ref() }) == value)
            .map(|ptr| unsafe { ptr.cast().as_ref() })
    }

    pub fn intern(&self, value: T) -> &T {
        let hash = FxBuildHasher.hash_one(&value);

        if let Some(cached) = self.try_resolve(&value) {
            return cached;
        }

        self.insert(hash, value)
    }
    pub(crate) fn insert(&self, hash: u64, value: T) -> &T {
        let arena = self.arena.get_or_init(Arena::new);

        let cached = NonNull::from(arena.alloc(value)).cast();
        self.set_mut().insert_unique(hash, cached, |t| FxBuildHasher.hash_one(t));
        unsafe { cached.cast().as_ref() }
    }
}

impl<T: fmt::Debug> fmt::Debug for Interner<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.set().fmt(f)
    }
}

unsafe impl<T> Send for Interner<T> where T: Send {}
