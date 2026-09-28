# Fabric Instance: HTTP Server

`onoal-fabric-instance-http-server` is a long-running operational Instance
artifact. It materializes the reusable HTTP Server Composition as a live
`fabric::Instance`, starts it, prints the observed listening address, and serves
requests until the operator terminates the process.

It is not Fabric runtime Instance persistence, a Composition, an Example, a
Profile, a deployment manager, a scheduler, or a supervisor.

## Run

From the repository root:

```bash
cargo run -p onoal-fabric-instance-http-server
```

Expected startup:

```text
HTTP server listening on http://127.0.0.1:8080
```

## Try It

From another terminal:

```bash
curl http://127.0.0.1:8080/example
```

Expected body:

```text
hello from /example
```

The server remains running. A second request works the same way:

```bash
curl http://127.0.0.1:8080/hello
```

Expected body:

```text
hello from /hello
```

## Stop

Stop the foreground process with `Ctrl+C`.

M16B does not install signal handling. `Ctrl+C` terminates the OS process; the
artifact does not yet guarantee a graceful `Instance::stop()` path for operator
termination.

## Architecture

```text
Instance artifact
    |
    v
HTTP Server Composition
    |
    v
TCP loopback realization + TcpTransportInspector + HttpServer
    |
    v
materialized fabric::Instance
```

The requested bind is `127.0.0.1:8080`, but runtime address truth comes from
`TcpTransportInspector`.

## Future Profile Seam

The current artifact owns one concrete operational default. Future Fabric
Profile-owned intent may lift defaults such as bind address, occurrence names,
or materialization choices without changing this artifact into a Profile API.
