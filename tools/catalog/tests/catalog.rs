#[test]
fn committed_catalog_output_is_current_for_actual_workspace() {
    fabric_ecosystem_catalog_tool::check_committed_catalog().expect("committed catalog");
}
