//! Asynchronous tickers backed by tokio's [`Interval`].
//!
//! A plain tokio [`Interval`] already implements [`AsyncTicker`], so you can
//! hand one to any code that is generic over `AsyncTicker`:
//!
//! ```no_run
//! use std::time::Duration;
//! use coolticker::AsyncTicker;
//!
//! #[tokio::main(flavor = "current_thread")]
//! async fn main() {
//!     let mut ticker = tokio::time::interval(Duration::from_millis(100));
//!     ticker.tick().await; // fires immediately
//!     ticker.tick().await; // ~100ms later
//! }
//! ```

use crate::AsyncTicker;
use ::tokio::time::Interval;

/// tokio's [`Interval`] already implements the [`AsyncTicker`] trait. This is just a type alias.
pub type IntervalTicker = Interval;

impl AsyncTicker for Interval {
    async fn tick(&mut self) {
        Interval::tick(self).await;
    }

    fn reset(&mut self) {
        Interval::reset(self);
    }
}

#[cfg(all(feature = "randomizable", not(target_family = "wasm")))]
mod random {
    use super::Interval;
    use crate::{AsyncTicker, Randomizable};
    use rand::SeedableRng;
    use rand::distr::{Distribution, Uniform};
    use rand::rngs::SmallRng;
    use std::time::Duration;

    /// An [`AsyncTicker`] backed by a real tokio [`Interval`] whose period can be
    /// re-rolled at runtime. Each call to [`Randomizable::randomize`] resamples
    /// the period uniformly from the configured `[min, max]` range and restarts
    /// the underlying interval, so the next tick fires one full (new) period from
    /// now rather than immediately.
    ///
    /// ```no_run
    /// use std::time::Duration;
    /// use coolticker::{AsyncTicker, Randomizable};
    /// use coolticker::tokio::RandomIntervalTicker;
    ///
    /// #[tokio::main(flavor = "current_thread")]
    /// async fn main() {
    ///     // Period is sampled uniformly from [50ms, 150ms].
    ///     let mut ticker = RandomIntervalTicker::new(
    ///         Duration::from_millis(50),
    ///         Duration::from_millis(150),
    ///     );
    ///
    ///     ticker.tick().await; // first tick
    ///     ticker.randomize();  // re-roll the period
    ///     ticker.tick().await; // next tick uses the new period
    /// }
    /// ```
    pub struct RandomIntervalTicker {
        interval: Interval,
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
            let mut rng = SmallRng::from_rng(&mut rand::rng());
            let period = Self::sample_period(&distribution, &mut rng);
            // Like `tokio::time::interval`, the first tick fires immediately.
            let interval = ::tokio::time::interval(period);
            Self {
                interval,
                distribution,
                rng,
            }
        }

        fn sample_period(distribution: &Uniform<u64>, rng: &mut SmallRng) -> Duration {
            // `tokio::time::interval` panics on a zero period, so floor at 1ms.
            let millis = distribution.sample(rng).max(1);
            Duration::from_millis(millis)
        }
    }

    impl AsyncTicker for RandomIntervalTicker {
        async fn tick(&mut self) {
            self.interval.tick().await;
        }

        fn reset(&mut self) {
            self.interval.reset();
        }
    }

    impl Randomizable for RandomIntervalTicker {
        fn randomize(&mut self) {
            let period = Self::sample_period(&self.distribution, &mut self.rng);
            // Schedule the first tick of the new interval one full period out, so
            // re-randomizing mid-run does not produce an immediate tick.
            self.interval =
                ::tokio::time::interval_at(::tokio::time::Instant::now() + period, period);
        }
    }
}

#[cfg(all(feature = "randomizable", not(target_family = "wasm")))]
pub use random::RandomIntervalTicker;
