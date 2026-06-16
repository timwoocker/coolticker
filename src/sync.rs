//! Synchronous tickers that pace a thread with [`std::thread::sleep`].

use crate::Ticker;
use std::time::{Duration, Instant};

/// A [`Ticker`] that fires at a fixed period by sleeping the current thread.
///
/// The first tick fires immediately. Subsequent tick times are anchored to a
/// schedule (rather than to when [`Ticker::tick`] returns), so processing time
/// between ticks does not drift the period.
///
/// ```no_run
/// use std::time::Duration;
/// use coolticker::Ticker;
/// use coolticker::sync::IntervalTicker;
///
/// let mut ticker = IntervalTicker::new(Duration::from_millis(100));
/// ticker.tick(); // fires immediately
/// std::thread::sleep(Duration::from_millis(50));
/// ticker.tick(); // blocks ~50ms (anchored to the schedule, so it won't drift)
/// ```
pub struct IntervalTicker {
    period: Duration,
    /// The instant the next tick is due.
    next: Instant,
}

impl IntervalTicker {
    pub fn new(period: Duration) -> Self {
        Self {
            period,
            // Schedule the first tick for "now" so it fires immediately, matching
            // the async ticker's behaviour.
            next: Instant::now(),
        }
    }

    /// Restart the ticker with a new period. Like [`Ticker::reset`], the next
    /// tick is scheduled one full period from now (it does not fire immediately).
    #[cfg(feature = "randomizable")]
    fn restart_with_period(&mut self, period: Duration) {
        self.period = period;
        self.next = Instant::now() + period;
    }
}

impl Ticker for IntervalTicker {
    fn tick(&mut self) {
        let now = Instant::now();
        if let Some(remaining) = self.next.checked_duration_since(now) {
            std::thread::sleep(remaining);
        }
        self.next += self.period;
    }

    fn reset(&mut self) {
        self.next = Instant::now() + self.period;
    }
}

#[cfg(feature = "randomizable")]
mod random {
    use super::IntervalTicker;
    use crate::{Randomizable, Ticker};
    use rand::SeedableRng;
    use rand::distr::{Distribution, Uniform};
    use rand::rngs::SmallRng;
    use std::time::Duration;

    /// A synchronous [`Ticker`] whose period can be re-rolled at runtime. Each
    /// call to [`Randomizable::randomize`] resamples the period uniformly from
    /// the configured `[min, max]` range and restarts the ticker, so the next
    /// tick fires one full (new) period from now rather than immediately.
    ///
    /// ```no_run
    /// use std::time::Duration;
    /// use coolticker::{Randomizable, Ticker};
    /// use coolticker::sync::RandomIntervalTicker;
    ///
    /// // Period is sampled uniformly from [50ms, 150ms].
    /// let mut ticker = RandomIntervalTicker::new(
    ///     Duration::from_millis(50),
    ///     Duration::from_millis(150),
    /// );
    ///
    /// ticker.tick();      // fires immediately
    /// ticker.randomize(); // re-roll the period
    /// ticker.tick();      // blocks for the new period (does not fire immediately)
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
            let mut rng = SmallRng::from_rng(&mut rand::rng());
            let period = Self::sample_period(&distribution, &mut rng);
            Self {
                inner: IntervalTicker::new(period),
                distribution,
                rng,
            }
        }

        fn sample_period(distribution: &Uniform<u64>, rng: &mut SmallRng) -> Duration {
            // Floor at 1ms to mirror the async ticker and avoid a zero period.
            let millis = distribution.sample(rng).max(1);
            Duration::from_millis(millis)
        }
    }

    impl Ticker for RandomIntervalTicker {
        fn tick(&mut self) {
            self.inner.tick();
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
