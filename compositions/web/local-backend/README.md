# Fabric Composition: Local Backend

`onoal-fabric-composition-local-backend` is an opinionated local backend
foundation. It reuses the HTTP Server Composition and adds local stateful
persistence and logging:

```text
HTTP Server Composition
    TCP loopback realization
    TcpTransportProbe
    HttpServer

SQLite RelationalDatabase
Console LogSink
```

This is a Composition, not a Package. It owns assembly opinion, not a new
Fabric semantic primitive.

```text
HTTP Server Composition
    HTTP-serving stack

Local Backend Composition
    HTTP-serving stack
    + SQLite relational persistence
    + Console logging
```

Application behavior remains consumer-owned. A consumer may add a Component
requiring `RelationalDatabase` and `LogSink`; this Composition does not create a
router, application handler, controller, repository, dependency-injection
container, ORM, or Component-to-Component service injection.

Use `local_backend_stack(config)` to contribute the foundation to a larger
Fabric build, or `build_local_backend_composition(id, config)` to build a
standalone ordinary `fabric::Composition`. The standalone builder reuses the
same stack helper.

The default local constructor uses loopback HTTP with an ephemeral port,
file-backed SQLite at the path you provide, and default Console logging.

Current Fabric v1 Component identity is definition-scoped. Because this stack
contains the HTTP Server Composition, the current law is one Local Backend HTTP
server stack per built Fabric Composition while all Resource names remain
explicit and configurable.

Non-goals: routing, middleware, JSON framework, authentication, authorization,
TLS, ORM, KeyValue cache, deployment, scheduling, CLI, registry, catalog
generator, or STEL integration.
