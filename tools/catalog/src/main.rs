fn main() {
    if let Err(error) = fabric_ecosystem_catalog_tool::run_from_env() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
