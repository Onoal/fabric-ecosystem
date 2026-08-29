# Fabric Service Resource

Service is a Resource for stable technical service identity and current
runtime target dispatch.

Service owns:

- stable `ResourceId("service")`
- `ServiceId`
- `ServiceEndpointId`
- `ServiceTargetId`
- `ServiceRequirement`
- `ServiceProtocol`
- target registration, readiness, draining, and withdrawal
- ready-target selection
- request dispatch semantics

Service does not own:

- ingress routes
- publication
- reachability
- naming
- authorization
- consumer-local access material

Service identities stay distinct:

- `ServiceId != ServiceEndpointId`
- `ServiceEndpointId != ServiceTargetId`
- `ServiceId != live endpoint`
- `ServiceId != URL`
- `ServiceId != IngressRoute`
- `ServiceId != Publication`
- `ServiceId != Reachability`

Service lifecycle and exposure remain separate truths:

- Service existence != Service availability
- Service availability != Service publication
- Service publication != Service reachability
- Service reachability != authorization

Current runtime state therefore looks like:

```text
ServiceId
    ↓
ServiceEndpointId
    ↓
current runtime targets
    ├── Registered
    ├── Ready
    └── Draining
```

Current consumer binding now targets stable `ServiceId` directly.

For the current workload consumer:

```text
WorkloadBinding
    ↓
ServiceId
    ↓
optional workload projection
    ↓
consumer-local access material
    ↓
Service dispatch
    ↓
resolve READY target now
    ↓
target runtime
```

This is a consumption graph, not an exposure graph.

Exposure remains separate:

```text
ServiceId
    ↓
IngressRoute
    ↓
Reachability
    ↓
Component publication or gateway semantics
```

Current Worker service projection is a narrow consumer-specific adapter above
Service. It may produce a loopback URL for today's workload runtime, but that
URL is not canonical Service truth and it does not pin a live endpoint.
Projection holds stable `ServiceId` and dispatches each request through the
Service contract so target replacement does not require rebinding or
reprojection.

A consumer-local projection URL is ephemeral access material only.

- its host is not Service identity
- its port is not Service identity
- its mount topology is not Service semantics

Projection routing prefixes must never become target-visible Service request
paths. A target must observe the same `ServiceHttpRequest` semantics whether a
request arrives through direct `ServiceContract::dispatch_http(...)` or through
consumer-local projected access.
