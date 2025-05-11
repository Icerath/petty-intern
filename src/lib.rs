pub mod unsync;

#[cfg(feature = "sync")]
pub mod sync;

pub use unsync::Interner;
