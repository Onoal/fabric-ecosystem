# Tests

`tests/` owns compatibility and integration verification.

This family is distinct from `examples/`:

```text
examples/ = teaching artifacts
tests/    = behavioral and public-consumer witnesses
```

Tests may be runnable, but their primary responsibility is preserving
cross-package behavior, public API compatibility, third-party consumption, and
regression coverage.

`tests/third-party-consumer` proves that ecosystem packages compose through
published public Fabric surfaces and package APIs rather than private
repository internals.
