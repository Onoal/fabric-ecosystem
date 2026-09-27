# Counter

`CounterMetric` is a Fabric Resource for one named monotonic unsigned counter.
It is aggregate telemetry: a value can stay the same or increase, but this
capability cannot decrement, reset, or set an absolute value.

## Public Model

- `current() -> Result<u64, CounterError>` reads the current value.
- `increment(amount) -> Result<CounterIncrementResult, CounterError>` attempts
  to add a `u64` amount.
- `CounterIncrementResult::Updated { value }` reports the new value.
- `CounterIncrementResult::Overflow { current, attempted }` reports a semantic
  overflow attempt and leaves the counter unchanged.
- `increment(0)` is a valid no-op and returns the current value as `Updated`.

`CounterError` is reserved for a live realization that cannot perform the read
or increment operation. Overflow is not an operation failure.

## In-Memory Realization

`InMemoryCounter` is the first realization. It stores a generation-local
`AtomicU64`, starts at `0` for each fresh materialization generation, and is
non-durable. Concurrent increments on one occurrence use atomic compare/exchange
so successful increments are not lost.

Use `in_memory_counter("requests")` to contribute one named counter occurrence.
Multiple named occurrences are independent.

## Consumer-Owned Meaning

The Counter package owns the generic metric capability. Consumers own what a
counter means and when to increment it. For example, an application can require
two `CounterMetric` Resources named `successful-sends` and `failed-sends`, but
those names and that success/failure interpretation are application behavior,
not generic Counter semantics.

## Boundaries

`CounterMetric` is not a Gauge, Histogram, Fabric Instance observation, health
state, log record, trace span, audit evidence, history, or identity record.
Counter state is live Adapter state, not Composition truth.

## Extension Path

Future work may add other metric capabilities, dimensional identity, persistent
or remote realizations, or export/scrape integrations. Those are not part of the
v1 Counter contract.

## Non-Goals

- decrement, reset, or arbitrary set
- labels/tags/dimensions
- gauge, histogram, timer, or floating-point metric semantics
- persistence guarantees
- Prometheus/OpenTelemetry exporter behavior
- aggregation across nodes
- audit/history/evidence semantics
