# onoal-fabric-process

`onoal-fabric-process` provides a reusable local operating-system process
capability for Fabric compositions.

It lets a Fabric module declaration start, observe, stop, and reap one local
child process occurrence when the Fabric runtime starts and stops.

It does not own:

- application identity;
- Base semantics;
- placement or scheduling;
- containers;
- service exposure;
- persistent storage;
- secrets;
- restart policy;
- a universal workload or execution ontology.

Process invocation is structured as `program` plus `argv`; the package does not
implicitly invoke a shell. The process runs with the permissions of the current
host process/user. This is not a sandbox.

