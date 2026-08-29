## ING0

Ingress is Fabric dataplane routing.

Canonical flow:

WorkerInstance
    -> ServiceTarget
    -> Service
    -> IngressRoute
    -> provider-owned local materialization
    -> Connectivity
    -> local reachability

Frozen laws:

- `IngressRoute` targets a stable `Service` identity.
- `Service` selects the current live `ServiceTarget` endpoint.
- `Ingress` does not select targets, publish names, authorize callers, or define reachability scope.
- `Service != current endpoint`.
- `IngressRouteId` is semantic and independent from listener address, localhost port, or URL material.
- Loopback listener ownership is provider machinery, not canonical ingress truth.
- `LocalHttpIngressAccess` is a narrow local/provider seam for current loopback compatibility.
- `Connectivity` owns reachability activation.
- Namespace does not belong to `Ingress`.
- Ingress does not imply publication, public Internet reachability, or authorization.
- Gateway remains above the Resource API.

Remember:

`Exists != Routed != Reachable != Published != Authorized`

## ING1

`fabric-adapter-ingress-pingora` is the current canonical ingress provider.

Frozen provider laws:

- Pingora provider ownership is machinery, not canonical ingress truth.
- The current default ingress posture remains loopback-only and local/private.
- Pingora listener address is runtime material and must not become route identity.
- `pingora = 0.8.1` is pinned exactly for ING1.
- Known dependency advisories are assessed and recorded, but a non-critical issue in the current local/private threat model does not automatically block Fabric progress.
- The currently known transitive advisory path is `pingora 0.8.1 -> pingora-core 0.8.1 -> pingora-pool 0.8.1 -> lru 0.16.4` (`RUSTSEC-2026-0253`).
- This advisory acceptance is limited to the current development posture: loopback-only ingress, no public Internet reachability, no untrusted remote clients, no multi-tenant exposure through this boundary, and no secret publication through the ingress provider.
- Before any Beta, LAN, remote, Gateway, or public reachability milestone, rerun dependency review, `cargo audit`, and Pingora upgrade evaluation.
