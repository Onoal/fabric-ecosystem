# Tests

This tree contains compatibility and integration fixtures that are not
ecosystem artifacts.

`third-party-consumer` is the public compatibility boundary. It is shaped like
an independent downstream crate and uses only public Fabric Ecosystem APIs. It
is intentionally excluded from Catalog projection because it is not a Package,
Host, Composition, or Example.
