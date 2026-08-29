# Fabric Connectivity Resource

Connectivity owns reachability.

Canonical Fabric flow:

- `WorkerInstance`
- `ServiceTarget`
- `ServiceEndpoint`
- `IngressRoute`
- `Reachability`

Frozen CONN0 rules:

- Connectivity answers under which `ConnectivityScope` an existing `IngressRoute` is reachable.
- `Ingress` owns dataplane routing and listener materialization.
- `Service` owns stable service and endpoint identity.
- Namespace belongs above the Resource API.
- Publication belongs above the Resource API.
- Authorization is separate.
- Sharing is separate.

Current v0 model:

- `ConnectivityScope::Local`
- `Reachability { id, ingress_route_id, scope }`
- `ReachabilityId` is derived only from `IngressRouteId + ConnectivityScope`

Durable truth:

- `ReachabilityId`
- `IngressRouteId`
- `ConnectivityScope`

Ephemeral truth:

- whether reachability is currently activated
- concrete loopback URL
- listener port
- provider handles

`LocalConnectivityAccess` is the narrow local/provider seam for current Home
and test reachability. It may contain a URL. Canonical connectivity semantics
must not.

Fabric distinctions that stay explicit:

- Exists != Routed
- Routed != Reachable
- Reachable != Named
- Named != Published
- Published != Authorized

Out of scope in CONN0:

- LAN
- overlay or tunnel scopes
- Gateway
- `.stel` DNS
- public Internet publication
