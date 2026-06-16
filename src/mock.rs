//! A deterministic ticker for tests, driven explicitly by a control handle.

use crate::{AsyncTicker, Ticker};

/// A side effect observed on a [`MockTicker`], forwarded to its
/// [`MockTickerControl`] so tests can react to it.
/// `Reset` is emitted when the ticker's `reset` method is called.
/// `RequestTick` is emitted when the ticker's `tick()` method is called (before the tick resolves).
/// `Randomize` is emitted when the ticker's `randomize()` method is called.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockTickerEvent {
    Reset,
    RequestTick,
    #[cfg(feature = "randomizable")]
    Randomize,
}

/// A deterministic [`Ticker`] / [`AsyncTicker`] for tests.
///
/// It never ticks on its own — a paired [`MockTickerControl`] advances it
/// explicitly. `reset` and `randomize` calls are recorded and sent to the
/// control handle so a test can observe and react to them. The same mock can be
/// used through either the synchronous [`Ticker`] or asynchronous
/// [`AsyncTicker`] interface.
///
/// ```
/// use coolticker::AsyncTicker;
/// use coolticker::mock::{MockTicker, MockTickerEvent};
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() {
/// let (mut ticker, control) = MockTicker::new();
///
/// // Drive the code under test in the background.
/// let task = tokio::spawn(async move {
///     ticker.tick().await; // resolves only once the control triggers a tick
///     "ticked"
/// });
///
/// // The ticker asks for a tick; deliver exactly one.
/// assert_eq!(control.next_event().await, Some(MockTickerEvent::RequestTick));
/// control.tick().unwrap();
///
/// assert_eq!(task.await.unwrap(), "ticked");
/// # }
/// ```
pub struct MockTicker {
    tick_rx: flume::Receiver<()>,
    events_tx: flume::Sender<MockTickerEvent>,
}

/// Test-side handle for driving and observing a [`MockTicker`].
pub struct MockTickerControl {
    tick_tx: flume::Sender<()>,
    events_rx: flume::Receiver<MockTickerEvent>,
}

impl MockTicker {
    /// Create a mock ticker together with the control handle used to drive it.
    pub fn new() -> (Self, MockTickerControl) {
        let (tick_tx, tick_rx) = flume::unbounded();
        let (events_tx, events_rx) = flume::unbounded();
        (
            Self { tick_rx, events_tx },
            MockTickerControl { tick_tx, events_rx },
        )
    }
}

impl AsyncTicker for MockTicker {
    async fn tick(&mut self) {
        // Resolve only when the test advances the ticker. If the control handle
        // is gone, instantly tick to prevent deadlocks.
        let _ = self
            .events_tx
            .send_async(MockTickerEvent::RequestTick)
            .await;
        let _ = self.tick_rx.recv_async().await;
    }

    fn reset(&mut self) {
        let _ = self.events_tx.send(MockTickerEvent::Reset);
    }
}

impl Ticker for MockTicker {
    fn tick(&mut self) {
        // Block until the test advances the ticker. If the control handle is
        // gone, instantly tick to prevent deadlocks.
        let _ = self.events_tx.send(MockTickerEvent::RequestTick);
        let _ = self.tick_rx.recv();
    }

    fn reset(&mut self) {
        let _ = self.events_tx.send(MockTickerEvent::Reset);
    }
}

#[cfg(feature = "randomizable")]
impl crate::Randomizable for MockTicker {
    fn randomize(&mut self) {
        let _ = self.events_tx.send(MockTickerEvent::Randomize);
    }
}

impl MockTickerControl {
    /// Trigger exactly one tick on the associated [`MockTicker`].
    /// When the receiving ticker is dropped, an `Err(...)` will be returned.
    pub fn tick(&self) -> Result<(), flume::SendError<()>> {
        self.tick_tx.send(())
    }

    /// Await the next [`MockTickerEvent`] emitted by the
    /// ticker. Returns `None` once the ticker has been dropped.
    pub async fn next_event(&self) -> Option<MockTickerEvent> {
        self.events_rx.recv_async().await.ok()
    }

    /// Await the next [`MockTickerEvent`] emitted by the
    /// ticker synchronously. Returns `None` once the ticker has been dropped.
    pub fn next_event_blocking(&self) -> Option<MockTickerEvent> {
        self.events_rx.recv().ok()
    }

    /// Return the next recorded event without waiting, if one is queued.
    pub fn try_next_event(&self) -> Option<MockTickerEvent> {
        self.events_rx.try_recv().ok()
    }
}
