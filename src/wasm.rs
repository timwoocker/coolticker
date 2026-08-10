//! Asynchronous tickers for `wasm32` targets, paced by the JavaScript
//! `setTimeout` timer.
//!
//! These work anywhere a global `setTimeout` and `performance` exist. They are
//! safe to await on the event loop.
//!
//! Because JavaScript timers are not [`Send`], neither are these tickers. On
//! `wasm` the [`AsyncTicker`] trait drops its [`Send`] bounds accordingly; see
//! [`MaybeSend`](crate::MaybeSend).

use crate::AsyncTicker;
use gloo_timers::future::TimeoutFuture;
use std::time::Duration;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
extern "C" {
    /// `performance.now()`: milliseconds on a monotonic clock. Unlike
    /// `Date.now()` it is immune to wall-clock adjustments, so a backwards NTP
    /// step cannot stall a ticker.
    #[wasm_bindgen(js_namespace = performance, js_name = now)]
    fn now_millis() -> f64;
}

/// An [`AsyncTicker`] that fires at a fixed period by awaiting a JavaScript
/// `setTimeout` between ticks.
///
/// The first tick fires immediately. Subsequent tick times are anchored to a
/// schedule (rather than to when [`AsyncTicker::tick`] resolves), so processing
/// time between ticks does not drift the period.
///
/// ```no_run
/// use std::time::Duration;
/// use coolticker::AsyncTicker;
/// use coolticker::wasm::IntervalTicker;
///
/// # async fn run() {
/// let mut ticker = IntervalTicker::new(Duration::from_millis(100));
/// ticker.tick().await; // fires immediately
/// ticker.tick().await; // ~100ms later
/// # }
/// ```
pub struct IntervalTicker {
    period_millis: f64,
    /// The time the next tick is due, on the `performance.now()` clock.
    next_millis: f64,
}

impl IntervalTicker {
    /// Create a ticker with the given period. The first tick fires immediately.
    ///
    /// # Panics
    ///
    /// Panics if `period` is zero, or longer than `u32::MAX` milliseconds
    /// (~49.7 days), which is the largest delay `setTimeout` accepts.
    pub fn new(period: Duration) -> Self {
        let period_millis = Self::checked_period_millis(period);
        Self {
            period_millis,
            // Schedule the first tick for "now" so it fires immediately,
            // matching the other ticker implementations.
            next_millis: now_millis(),
        }
    }

    fn checked_period_millis(period: Duration) -> f64 {
        let millis = period.as_secs_f64() * 1000.0;
        assert!(
            millis > 0.0 && millis <= u32::MAX as f64,
            "period must be non-zero and at most u32::MAX milliseconds"
        );
        millis
    }

    /// Restart the ticker with a new period. Like [`AsyncTicker::reset`], the
    /// next tick is scheduled one full period from now (it does not fire
    /// immediately).
    fn restart_with_period(&mut self, period: Duration) {
        self.period_millis = Self::checked_period_millis(period);
        self.next_millis = now_millis() + self.period_millis;
    }
}

impl AsyncTicker for IntervalTicker {
    async fn tick(&mut self) {
        let remaining = self.next_millis - now_millis();
        if remaining > 0.0 {
            // `setTimeout` takes whole milliseconds; round up so a tick is never
            // delivered early. The anchored schedule absorbs the rounding.
            TimeoutFuture::new(remaining.ceil() as u32).await;
        }
        self.next_millis += self.period_millis;
    }

    fn reset(&mut self) {
        self.next_millis = now_millis() + self.period_millis;
    }
}

#[cfg(feature = "randomizable")]
mod random {
    use super::{IntervalTicker, now_millis};
    use crate::{AsyncTicker, Randomizable};
    use rand::SeedableRng;
    use rand::distr::{Distribution, Uniform};
    use rand::rngs::SmallRng;
    use std::time::Duration;

    /// An [`AsyncTicker`] backed by a [`IntervalTicker`] whose period can be
    /// re-rolled at runtime. Each call to [`Randomizable::randomize`] resamples
    /// the period uniformly from the configured `[min, max]` range and restarts
    /// the ticker, so the next tick fires one full (new) period from now rather
    /// than immediately.
    ///
    /// ```no_run
    /// use std::time::Duration;
    /// use coolticker::{AsyncTicker, Randomizable};
    /// use coolticker::wasm::RandomIntervalTicker;
    ///
    /// # async fn run() {
    /// // Period is sampled uniformly from [50ms, 150ms].
    /// let mut ticker = RandomIntervalTicker::new(
    ///     Duration::from_millis(50),
    ///     Duration::from_millis(150),
    /// );
    ///
    /// ticker.tick().await; // first tick
    /// ticker.randomize();  // re-roll the period
    /// ticker.tick().await; // next tick uses the new period
    /// # }
    /// ```
    pub struct RandomIntervalTicker {
        /// The underlying fixed-period ticker; `randomize` swaps its period.
        inner: IntervalTicker,
        /// Distribution of period lengths, in whole milliseconds, that each tick
        /// interval is sampled from.
        distribution: Uniform<u64>,
        rng: SmallRng,
    }

    impl RandomIntervalTicker {
        pub fn new(min: Duration, max: Duration) -> Self {
            let distribution =
                Uniform::new_inclusive(min.as_millis() as u64, max.as_millis() as u64)
                    .expect("invalid period bounds: min must be <= max");
            let mut rng = SmallRng::seed_from_u64(js_seed());
            let period = Self::sample_period(&distribution, &mut rng);
            Self {
                inner: IntervalTicker::new(period),
                distribution,
                rng,
            }
        }

        fn sample_period(distribution: &Uniform<u64>, rng: &mut SmallRng) -> Duration {
            // Floor at 1ms to mirror the other tickers and avoid a zero period.
            let millis = distribution.sample(rng).max(1);
            Duration::from_millis(millis)
        }
    }

    /// Draw a seed from JavaScript rather than from `rand::rng()`.
    fn js_seed() -> u64 {
        // `Math.random` yields ~53 bits at best, so combine two draws. Mixing in
        // the clock keeps two tickers built in the same turn of the event loop
        // apart even if the host's PRNG is coarse.
        let high = (js_sys::Math::random() * (1u64 << 32) as f64) as u64;
        let low = (js_sys::Math::random() * (1u64 << 32) as f64) as u64;
        (high << 32) ^ low ^ (now_millis().to_bits())
    }

    impl AsyncTicker for RandomIntervalTicker {
        async fn tick(&mut self) {
            self.inner.tick().await;
        }

        fn reset(&mut self) {
            self.inner.reset();
        }
    }

    impl Randomizable for RandomIntervalTicker {
        fn randomize(&mut self) {
            let period = Self::sample_period(&self.distribution, &mut self.rng);
            self.inner.restart_with_period(period);
        }
    }
}

#[cfg(feature = "randomizable")]
pub use random::RandomIntervalTicker;
