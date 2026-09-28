use std::sync::atomic::{AtomicU64, Ordering};

use crate::{CounterError, CounterIncrementResult, CounterMetric};

#[derive(Default)]
struct InMemoryCounterState {
    value: AtomicU64,
}

impl InMemoryCounterState {
    fn current(&self) -> Result<u64, CounterError> {
        Ok(self.value.load(Ordering::SeqCst))
    }

    fn increment(&self, amount: u64) -> Result<CounterIncrementResult, CounterError> {
        let mut current = self.value.load(Ordering::SeqCst);
        loop {
            let Some(next) = current.checked_add(amount) else {
                return Ok(CounterIncrementResult::Overflow {
                    current,
                    attempted: amount,
                });
            };
            match self
                .value
                .compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return Ok(CounterIncrementResult::Updated { value: next }),
                Err(observed) => current = observed,
            }
        }
    }
}

fabric::adapter! {
    pub InMemoryCounter for CounterMetric {
        id: "onoal.package.observability.counter.in-memory";

        state {
            InMemoryCounterState = InMemoryCounterState::default();
        }

        runtime {
            async fn current(&self) -> Result<u64, CounterError> {
                self.state.get().current()
            }

            async fn increment(&self, amount: u64) -> Result<CounterIncrementResult, CounterError> {
                self.state.get().increment(amount)
            }
        }
    }
}
