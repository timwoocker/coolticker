#![doc = include_str!("../README.md")]

/// [`Send`] on every target except `wasm`, where it is an empty bound.
///
/// Browser timers hand a callback to JavaScript, so the futures built on them
/// hold `JsValue`s and can never be [`Send`]. Requiring [`Send`] unconditionally
/// would put the `wasm` module's tickers out of reach of the ticker traits, so
/// the bound is relaxed on `wasm` and kept everywhere else.
#[cfg(not(target_family = "wasm"))]
pub trait MaybeSend: Send {}
#[cfg(not(target_family = "wasm"))]
impl<T: Send + ?Sized> MaybeSend for T {}

#[cfg(target_family = "wasm")]
pub trait MaybeSend {}
#[cfg(target_family = "wasm")]
impl<T: ?Sized> MaybeSend for T {}

/// Abstraction over a synchronous periodic ticker.
pub trait Ticker: MaybeSend + 'static {
    /// Block until the next tick.
    fn tick(&mut self);

    /// Reset the ticker so the next tick happens one full period from now.
    fn reset(&mut self);
}

/// Abstraction over an asynchronous periodic ticker (like a tokio `Interval`).
pub trait AsyncTicker: MaybeSend + 'static {
    /// Wait for the next tick.
    fn tick(&mut self) -> impl Future<Output = ()> + MaybeSend;

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

#[cfg(all(feature = "wasm", target_family = "wasm"))]
pub mod wasm;
