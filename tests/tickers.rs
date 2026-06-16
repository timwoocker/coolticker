//! Behavioural tests for the interval tickers and the mock.
//!
//! Each module is gated on the features its types require, so the test crate
//! compiles under any feature selection (and the relevant tests run when those
//! features are enabled, e.g. with `--all-features`).

#[cfg(feature = "sync")]
mod sync_ticker {
    use coolticker::Ticker;
    use coolticker::sync::IntervalTicker;
    use std::time::{Duration, Instant};

    #[test]
    fn first_tick_is_immediate() {
        let mut ticker = IntervalTicker::new(Duration::from_millis(300));
        let start = Instant::now();
        ticker.tick();
        assert!(
            start.elapsed() < Duration::from_millis(10),
            "first tick should fire immediately"
        );
    }

    #[test]
    fn subsequent_tick_waits_one_period() {
        let mut ticker = IntervalTicker::new(Duration::from_millis(150));
        let start = Instant::now();
        ticker.tick(); // immediate
        ticker.tick();
        assert!(
            start.elapsed() >= Duration::from_millis(150),
            "second tick should wait ~one period"
        );
    }
}

#[cfg(all(feature = "sync", feature = "randomizable"))]
mod sync_random_ticker {
    use coolticker::sync::RandomIntervalTicker;
    use coolticker::{Randomizable, Ticker};
    use std::time::{Duration, Instant};

    // A fixed period (min == max) keeps the timing assertions deterministic.
    fn fixed(period_ms: u64) -> RandomIntervalTicker {
        RandomIntervalTicker::new(
            Duration::from_millis(period_ms),
            Duration::from_millis(period_ms),
        )
    }

    #[test]
    fn first_tick_is_immediate() {
        let mut ticker = fixed(300);
        let start = Instant::now();
        ticker.tick();
        assert!(
            start.elapsed() < Duration::from_millis(100),
            "first tick should fire immediately"
        );
    }

    #[test]
    fn randomize_restarts_without_instant_tick() {
        let mut ticker = fixed(150);
        ticker.tick(); // immediate
        ticker.randomize();
        let start = Instant::now();
        ticker.tick(); // restarted: must wait ~one full period, not fire instantly
        assert!(
            start.elapsed() >= Duration::from_millis(120),
            "randomize must restart the schedule, not tick instantly"
        );
    }
}

#[cfg(all(feature = "tokio", feature = "randomizable"))]
mod tokio_random_ticker {
    use coolticker::tokio::RandomIntervalTicker;
    use coolticker::{AsyncTicker, Randomizable};
    use tokio::time::{Duration, Instant};

    // we don't test tokio's `Interval` itself, only our random wrapper

    // A fixed period (min == max) keeps the timing assertions deterministic.
    fn fixed(period_ms: u64) -> RandomIntervalTicker {
        RandomIntervalTicker::new(
            Duration::from_millis(period_ms),
            Duration::from_millis(period_ms),
        )
    }

    #[tokio::test(start_paused = true)]
    async fn first_tick_is_immediate() {
        let mut ticker = fixed(100);
        let start = Instant::now();
        ticker.tick().await;
        assert_eq!(
            start.elapsed(),
            Duration::ZERO,
            "first tick should fire immediately"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn randomize_restarts_without_instant_tick() {
        let mut ticker = fixed(100);
        ticker.tick().await; // immediate
        ticker.randomize();
        let start = Instant::now();
        ticker.tick().await; // restarted: next tick one full period out
        assert_eq!(
            start.elapsed(),
            Duration::from_millis(100),
            "randomize must restart the schedule, not tick instantly"
        );
    }
}

#[cfg(feature = "mock")]
mod mock_ticker {
    use coolticker::AsyncTicker;
    use coolticker::mock::{MockTicker, MockTickerEvent};

    #[tokio::test]
    async fn tick_is_driven_by_control_and_records_events() {
        let (mut ticker, control) = MockTicker::new();

        ticker.reset();
        assert_eq!(control.next_event().await, Some(MockTickerEvent::Reset));

        // `tick` only resolves once the control delivers a tick.
        let task = tokio::spawn(async move {
            ticker.tick().await;
        });
        assert_eq!(
            control.next_event().await,
            Some(MockTickerEvent::RequestTick)
        );
        control.tick().unwrap();
        task.await.unwrap();
    }

    #[cfg(feature = "randomizable")]
    #[tokio::test]
    async fn randomize_records_event() {
        use coolticker::Randomizable;

        let (mut ticker, control) = MockTicker::new();
        ticker.randomize();
        assert_eq!(
            control.next_event().await,
            Some(MockTickerEvent::Randomize)
        );
    }
}
