#![doc = include_str!("../README.md")]

/// Abstraction over a synchronous periodic ticker.
pub trait Ticker: Send + 'static {
    /// Block until the next tick.
    fn tick(&mut self);

    /// Reset the ticker so the next tick happens one full period from now.
    fn reset(&mut self);
}

/// Abstraction over an asynchronous periodic ticker (like a tokio `Interval`).
pub trait AsyncTicker: Send + 'static {
    /// Wait for the next tick.
    fn tick(&mut self) -> impl Future<Output = ()> + Send;

    /// Reset the ticker so the next tick happens one full period from now.
    fn reset(&mut self);
}

/// A ticker whose period can be re-rolled randomly at runtime.
#[cfg(feature = "randomizable")]
pub trait Randomizable {
    /// Pick a new (random) period to use for subsequent ticks.
    fn randomize(&mut self);
}

#[cfg(feature = "sync")]
pub mod sync;

#[cfg(feature = "tokio")]
pub mod tokio;

#[cfg(feature = "mock")]
pub mod mock;
