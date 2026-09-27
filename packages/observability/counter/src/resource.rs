use crate::{CounterError, CounterIncrementResult};

fabric::resource! {
    pub CounterMetric {
        id: "onoal.package.observability.counter.metric";

        api {
            fn current(&self) -> Result<u64, CounterError>;
            fn increment(&self, amount: u64) -> Result<CounterIncrementResult, CounterError>;
        }
    }
}
