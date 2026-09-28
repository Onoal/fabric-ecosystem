#[test]
fn committed_catalog_output_is_current_for_actual_workspace() {
    fabric_ecosystem_catalog_tool::check_committed_catalog().expect("committed catalog");
}

#[test]
fn committed_catalog_can_be_consumed_without_source_discovery() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root");
    let catalog =
        fabric_ecosystem_catalog_tool::read_committed_catalog_from(root).expect("catalog");

    assert_eq!(
        catalog
            .artifact_by_path("packages/networking/http")
            .expect("http package")
            .cargo_package,
        "onoal-fabric-package-networking-http"
    );
    assert_eq!(
        catalog
            .artifact_by_path("hosts/linux")
            .expect("linux host")
            .cargo_package,
        "onoal-fabric-host-linux"
    );
    assert_eq!(
        catalog
            .artifact_by_path("compositions/web/local-backend")
            .expect("local backend")
            .cargo_package,
        "onoal-fabric-composition-local-backend"
    );
    assert_eq!(
        catalog
            .artifact_by_path("examples/ed25519-signing")
            .expect("ed25519 example")
            .cargo_package,
        "fabric-ecosystem-example-ed25519-signing"
    );
    assert_eq!(
        catalog
            .artifact_by_path("instances/web/http-server")
            .expect("http server instance")
            .cargo_package,
        "onoal-fabric-instance-http-server"
    );

    let http_server_dependencies = catalog
        .direct_dependencies("compositions/web/http-server")
        .expect("dependencies");
    assert_eq!(
        http_server_dependencies.normal,
        vec![
            "packages/networking/http".to_owned(),
            "packages/networking/tcp".to_owned()
        ]
    );

    let tcp_dependents = catalog
        .direct_dependents("packages/networking/tcp")
        .expect("dependents");
    assert!(tcp_dependents.iter().any(|dependent| {
        dependent.artifact.path == "compositions/web/http-server"
            && dependent.kind == fabric_ecosystem_catalog_tool::DependencyKind::Normal
    }));
    assert!(tcp_dependents.iter().any(|dependent| {
        dependent.artifact.path == "instances/web/http-server"
            && dependent.kind == fabric_ecosystem_catalog_tool::DependencyKind::Normal
    }));
}
