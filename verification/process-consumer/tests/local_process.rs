use std::fs;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use fabric::core::{
    BlockBuilder, BlockId, Composition, CompositionBuilder, CompositionId, ContractId, Health,
    InstanceId, ModuleId,
};
use fabric::host::HostDescriptor;
use onoal_fabric_process::{
    LocalProcessDefinition, LocalProcessExit, LocalProcessHandle, LocalProcessModule,
    LocalProcessStatus, local_process_export,
};
use tempfile::TempDir;

fn helper() -> String {
    env!("CARGO_BIN_EXE_process-helper").to_owned()
}

fn module_id(name: &str) -> ModuleId {
    ModuleId::new(format!("test.process.{name}")).expect("module id")
}

fn composition_id(name: &str) -> CompositionId {
    CompositionId::new(format!("test.process.{name}.composition")).expect("composition id")
}

fn block_id(name: &str) -> BlockId {
    BlockId::new(format!("test.process.{name}.block")).expect("block id")
}

fn instance_id(name: &str) -> InstanceId {
    InstanceId::new(format!("test.process.{name}.instance")).expect("instance id")
}

fn export_id(name: &str) -> ContractId {
    ContractId::new(format!("test.process.{name}.export")).expect("export id")
}

fn process_composition(
    name: &str,
    args: Vec<String>,
) -> (
    Composition,
    fabric::core::CompositionExport<LocalProcessHandle>,
) {
    let module_id = module_id(name);
    let definition =
        LocalProcessDefinition::with_args(module_id.clone(), helper(), args).expect("definition");
    let module = LocalProcessModule::new(definition);
    let export = local_process_export(export_id(name), &module_id);
    let composition = CompositionBuilder::new(composition_id(name))
        .register_block(
            BlockBuilder::new(block_id(name))
                .register_module(module)
                .build(),
        )
        .export(export.clone())
        .build()
        .expect("composition");
    (composition, export)
}

fn materialize(
    name: &str,
    composition: &Composition,
    export: &fabric::core::CompositionExport<LocalProcessHandle>,
) -> (fabric::core::Instance, Arc<LocalProcessHandle>) {
    let instance = composition
        .materialize_on(instance_id(name), &HostDescriptor::native())
        .expect("materialize on native host");
    let handle = instance.export(export).expect("process handle export");
    (instance, handle)
}

fn wait_until<F>(message: &str, mut condition: F)
where
    F: FnMut() -> bool,
{
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if condition() {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("{message}");
}

fn wait_for_status(
    handle: &LocalProcessHandle,
    predicate: impl Fn(&LocalProcessStatus) -> bool,
) -> LocalProcessStatus {
    let mut last = None;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let status = handle.observe().expect("observe").status().clone();
        if predicate(&status) {
            return status;
        }
        last = Some(status);
        thread::sleep(Duration::from_millis(20));
    }
    panic!("status did not converge, last status: {last:?}");
}

fn marker_values(path: &std::path::Path) -> (u32, String) {
    let text = fs::read_to_string(path).expect("marker");
    let mut lines = text.lines();
    let pid = lines.next().expect("pid").parse().expect("pid");
    let token = lines.next().expect("token").to_owned();
    (pid, token)
}

#[test]
fn real_start_and_stop_owns_and_reaps_child_process() {
    let temp = TempDir::new().expect("tempdir");
    let marker = temp.path().join("process.marker");
    let (composition, export) = process_composition(
        "start-stop",
        vec!["long-running".to_owned(), marker.display().to_string()],
    );
    let (mut instance, handle) = materialize("start-stop", &composition, &export);

    assert_eq!(
        handle.observe().expect("observe").status(),
        &LocalProcessStatus::NotStarted
    );
    instance.start().expect("start instance");
    wait_until("helper did not write marker", || marker.exists());
    let (marker_pid, _) = marker_values(&marker);
    let status = wait_for_status(&handle, |status| {
        matches!(status, LocalProcessStatus::Running { .. })
    });
    assert_eq!(status, LocalProcessStatus::Running { pid: marker_pid });
    assert_eq!(instance.report().health, Health::Healthy);

    instance.stop();
    let stopped = wait_for_status(&handle, |status| {
        matches!(status, LocalProcessStatus::Stopped { .. })
    });
    assert_eq!(
        stopped,
        LocalProcessStatus::Stopped {
            pid: Some(marker_pid)
        }
    );
}

#[test]
fn unsuccessful_exit_is_observable_and_not_restarted() {
    let (composition, export) = process_composition("failure", vec!["exit-failure".to_owned()]);
    let (mut instance, handle) = materialize("failure", &composition, &export);

    instance.start().expect("start failure helper");
    let status = wait_for_status(&handle, |status| {
        matches!(
            status,
            LocalProcessStatus::Exited {
                exit: LocalProcessExit { .. },
                ..
            }
        )
    });
    match status {
        LocalProcessStatus::Exited { exit, .. } => {
            assert_eq!(exit.code(), Some(17));
            assert!(!exit.successful());
        }
        other => panic!("unexpected status: {other:?}"),
    }
    assert_eq!(instance.report().health, Health::Unavailable);
}

#[test]
fn successful_exit_is_completion_not_running_or_failure() {
    let (composition, export) = process_composition("success", vec!["exit-success".to_owned()]);
    let (mut instance, handle) = materialize("success", &composition, &export);

    instance.start().expect("start success helper");
    let status = wait_for_status(&handle, |status| {
        matches!(
            status,
            LocalProcessStatus::Exited {
                exit: LocalProcessExit { .. },
                ..
            }
        )
    });
    match status {
        LocalProcessStatus::Exited { exit, .. } => {
            assert_eq!(exit.code(), Some(0));
            assert!(exit.successful());
        }
        other => panic!("unexpected status: {other:?}"),
    }
    assert_eq!(instance.report().health, Health::Degraded);
}

#[test]
fn rematerialization_creates_a_new_os_process_occurrence() {
    let temp = TempDir::new().expect("tempdir");
    let first_marker = temp.path().join("first.marker");
    let second_marker = temp.path().join("second.marker");

    let (first_composition, first_export) = process_composition(
        "rematerialize",
        vec![
            "long-running".to_owned(),
            first_marker.display().to_string(),
        ],
    );
    let (mut first_instance, first_handle) =
        materialize("rematerialize-a", &first_composition, &first_export);
    first_instance.start().expect("start first");
    wait_until("first marker missing", || first_marker.exists());
    let first_observation = marker_values(&first_marker);
    first_instance.stop();
    wait_for_status(&first_handle, |status| {
        matches!(status, LocalProcessStatus::Stopped { .. })
    });

    let (second_composition, second_export) = process_composition(
        "rematerialize",
        vec![
            "long-running".to_owned(),
            second_marker.display().to_string(),
        ],
    );
    let (mut second_instance, second_handle) =
        materialize("rematerialize-b", &second_composition, &second_export);
    second_instance.start().expect("start second");
    wait_until("second marker missing", || second_marker.exists());
    let second_observation = marker_values(&second_marker);

    assert_ne!(
        first_observation, second_observation,
        "same declaration must create a new OS occurrence after rematerialization"
    );
    assert_ne!(first_instance.instance_id(), second_instance.instance_id());
    assert_ne!(first_instance.generation(), second_instance.generation());
    second_instance.stop();
    wait_for_status(&second_handle, |status| {
        matches!(status, LocalProcessStatus::Stopped { .. })
    });
}

#[test]
fn two_independent_processes_run_without_global_state_collision() {
    let temp = TempDir::new().expect("tempdir");
    let first_marker = temp.path().join("one.marker");
    let second_marker = temp.path().join("two.marker");
    let first_module = module_id("multi.one");
    let second_module = module_id("multi.two");

    let first = LocalProcessModule::new(
        LocalProcessDefinition::with_args(
            first_module.clone(),
            helper(),
            vec![
                "long-running".to_owned(),
                first_marker.display().to_string(),
            ],
        )
        .expect("first definition"),
    );
    let second = LocalProcessModule::new(
        LocalProcessDefinition::with_args(
            second_module.clone(),
            helper(),
            vec![
                "long-running".to_owned(),
                second_marker.display().to_string(),
            ],
        )
        .expect("second definition"),
    );
    let first_export = local_process_export(export_id("multi.one"), &first_module);
    let second_export = local_process_export(export_id("multi.two"), &second_module);
    let composition = CompositionBuilder::new(composition_id("multi"))
        .register_block(
            BlockBuilder::new(block_id("multi"))
                .register_module(first)
                .register_module(second)
                .build(),
        )
        .export(first_export.clone())
        .export(second_export.clone())
        .build()
        .expect("multi composition");
    let mut instance = composition
        .materialize_on(instance_id("multi"), &HostDescriptor::native())
        .expect("materialize");
    let first_handle = instance.export(&first_export).expect("first export");
    let second_handle = instance.export(&second_export).expect("second export");

    instance.start().expect("start both");
    wait_until("first marker", || first_marker.exists());
    wait_until("second marker", || second_marker.exists());
    let first_observation = marker_values(&first_marker);
    let second_observation = marker_values(&second_marker);
    assert_ne!(first_observation, second_observation);
    assert!(first_handle.observe().expect("first").is_running());
    assert!(second_handle.observe().expect("second").is_running());

    instance.stop();
    wait_for_status(&first_handle, |status| {
        matches!(status, LocalProcessStatus::Stopped { .. })
    });
    wait_for_status(&second_handle, |status| {
        matches!(status, LocalProcessStatus::Stopped { .. })
    });
}
