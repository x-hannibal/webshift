//! HTML fetching and cleaning pipeline.

pub mod cleaner;
pub mod fetcher;

#[cfg(feature = "text-map")]
pub mod textmap;
