# coolticker

A small ticker abstraction with sync, async (tokio), randomizable, and mockable interval implementations.

`coolticker` lets you write code that is generic over *how* it is paced. The same logic can be driven by a real clock in
production and by a deterministic mock in tests.

## Features

By default, only the core traits are available. Implementations can be enabled via the following features:

| Feature        | Enables                                                                             |
|----------------|-------------------------------------------------------------------------------------|
| `sync`         | `sync::IntervalTicker`                                                              |
| `tokio`        | `AsyncTicker` for tokio's `Interval`                                                |
| `wasm`         | `wasm::IntervalTicker`, for `wasm32` targets (browser, workers, Node)               |
| `randomizable` | the `Randomizable` trait and a `RandomIntervalTicker` in each implementation module |
| `mock`         | `mock::MockTicker`, a deterministic ticker for tests                                |

## Traits

- **`Ticker`** — synchronous ticker.
- **`AsyncTicker`** — asynchronous ticker.
- **`Randomizable`** — orthogonal to ticking. Allows manually re-rolling the tick interval. Combine it with either
  ticker trait in your bounds, e.g. `T: AsyncTicker + Randomizable`.

## Usage

Write your logic generic over a ticker trait, then pass a real ticker in production and a `MockTicker` in tests — the
function under test never changes.

```rust
use coolticker::AsyncTicker;

/// Polls `is_ready` once per tick, returning how many ticks it took to become
/// ready.
async fn wait_until_ready(
    mut ticker: impl AsyncTicker,
    mut is_ready: impl FnMut() -> bool,
) -> usize {
    let mut ticks = 0;
    loop {
        ticker.tick().await;
        ticks += 1;
        if is_ready() {
            return ticks;
        }
    }
}
```

### In production

A tokio `Interval` implements `AsyncTicker` (with the `tokio` feature), so you can pass one straight in:

```rust,ignore
use std::time::Duration;

let ticker = tokio::time::interval(Duration::from_secs(1));
let ticks = wait_until_ready(ticker, || service_is_up()).await;
```

### In tests

Drive the very same function with a `MockTicker` (with the `mock` feature):

```rust
use coolticker::mock::MockTicker;

#[tokio::test]
async fn becomes_ready_after_three_ticks() {
    let (ticker, control) = MockTicker::new();

    // Run the function under test in the background; report ready on the 3rd poll.
    let mut polls = 0;
    let task = tokio::spawn(wait_until_ready(ticker, move || {
        polls += 1;
        polls == 3
    }));

    // Advance "time" by exactly three ticks.
    for _ in 0..3 {
        control.tick().unwrap();
    }

    assert_eq!(task.await.unwrap(), 3);
}
```

`MockTicker` also implements the synchronous `Ticker` trait, and it records
`reset`/`randomize` calls as events you can assert on via the control handle.

## In the browser

With the `wasm` feature, `wasm::IntervalTicker` paces the same generic code on
`wasm32` targets by awaiting JavaScript's `setTimeout`. It binds the global
`setTimeout` and `performance`, so it works on the main thread, in web workers and under Node:

```rust,ignore
use std::time::Duration;
use coolticker::wasm::IntervalTicker;

let ticker = IntervalTicker::new(Duration::from_secs(1));
let ticks = wait_until_ready(ticker, || service_is_up()).await;
```

One thing behaves differently there: **the ticker traits drop their `Send`
bounds on `wasm`.** A `setTimeout` future owns a JavaScript callback and can never be `Send`, so requiring it would make
browser tickers unimplementable. The bounds are expressed through the blanket-implemented `MaybeSend` trait, which is
`Send` on every other target — native code sees no change.
