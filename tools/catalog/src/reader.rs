use std::fs;
use std::path::Path;

use crate::error::CatalogError;
use crate::model::CatalogIndex;
use crate::query::Catalog;
use crate::validation;

const JSON_PATH: &str = "catalog/index.json";

pub fn read_catalog_from(root: &Path) -> Result<Catalog, CatalogError> {
    let content = fs::read_to_string(root.join(JSON_PATH))
        .map_err(|error| CatalogError::new(format!("failed to read {JSON_PATH}: {error}")))?;
    read_catalog_str(&content)
}

pub(crate) fn read_catalog_str(content: &str) -> Result<Catalog, CatalogError> {
    let index: CatalogIndex = serde_json::from_str(content)
        .map_err(|error| CatalogError::new(format!("invalid JSON in {JSON_PATH}: {error}")))?;
    validation::validate_catalog_index(&index)?;
    Ok(Catalog::from_validated(index))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_catalog() -> &'static str {
        r#"{
  "schemaVersion": 1,
  "artifacts": [
    {
      "kind": "package",
      "category": "networking",
      "path": "packages/networking/http",
      "cargoPackage": "onoal-fabric-package-networking-http",
      "version": "0.1.0",
      "description": "HTTP",
      "publishable": true,
      "dependencies": {
        "normal": [
          "packages/networking/tcp"
        ],
        "development": [],
        "build": []
      }
    },
    {
      "kind": "package",
      "category": "networking",
      "path": "packages/networking/tcp",
      "cargoPackage": "onoal-fabric-package-networking-tcp",
      "version": "0.1.0",
      "description": "TCP",
      "publishable": true,
      "dependencies": {
        "normal": [],
        "development": [],
        "build": []
      }
    }
  ]
}"#
    }

    fn valid_catalog_v2() -> &'static str {
        r#"{
  "schemaVersion": 2,
  "artifacts": [
    {
      "kind": "instance",
      "category": "web",
      "path": "instances/web/http-server",
      "cargoPackage": "onoal-fabric-instance-http-server",
      "version": "0.1.0",
      "description": "HTTP server Instance",
      "publishable": true,
      "dependencies": {
        "normal": [],
        "development": [],
        "build": []
      }
    }
  ]
}"#
    }

    #[test]
    fn valid_catalog_v1_loads() {
        let catalog = read_catalog_str(valid_catalog()).expect("catalog");
        assert_eq!(catalog.artifacts().len(), 2);
        assert_eq!(catalog.schema_version(), 1);
    }

    #[test]
    fn valid_catalog_v2_with_instance_loads() {
        let catalog = read_catalog_str(valid_catalog_v2()).expect("catalog");
        assert_eq!(catalog.artifacts().len(), 1);
        assert_eq!(catalog.schema_version(), 2);
        assert_eq!(
            catalog
                .artifact_by_path("instances/web/http-server")
                .expect("instance")
                .cargo_package,
            "onoal-fabric-instance-http-server"
        );
    }

    #[test]
    fn catalog_v1_rejects_instance_kind() {
        let document = valid_catalog_v2().replace("\"schemaVersion\": 2", "\"schemaVersion\": 1");
        assert!(read_catalog_str(&document)
            .unwrap_err()
            .to_string()
            .contains("invalid Catalog v1 topology"));
    }

    #[test]
    fn unsupported_schema_version_rejects() {
        let document = valid_catalog().replace("\"schemaVersion\": 1", "\"schemaVersion\": 3");
        assert_eq!(
            read_catalog_str(&document).unwrap_err().to_string(),
            "unsupported Catalog schema version: 3"
        );
    }

    #[test]
    fn invalid_json_is_bounded() {
        assert!(read_catalog_str("{")
            .unwrap_err()
            .to_string()
            .contains("invalid JSON"));
    }

    #[test]
    fn unknown_top_level_fields_reject() {
        let document = valid_catalog().replace(
            "\"schemaVersion\": 1,",
            "\"schemaVersion\": 1,\n  \"maturity\": \"stable\",",
        );
        assert!(read_catalog_str(&document)
            .unwrap_err()
            .to_string()
            .contains("unknown field `maturity`"));
    }

    #[test]
    fn unknown_artifact_fields_reject() {
        let document = valid_catalog().replace(
            "\"kind\": \"package\",",
            "\"kind\": \"package\",\n      \"provides\": [\"TcpByteStreamTransport\"],",
        );
        assert!(read_catalog_str(&document)
            .unwrap_err()
            .to_string()
            .contains("unknown field `provides`"));
    }

    #[test]
    fn unknown_dependency_fields_reject() {
        let document =
            valid_catalog().replace("\"build\": []", "\"build\": [],\n        \"requires\": []");
        assert!(read_catalog_str(&document)
            .unwrap_err()
            .to_string()
            .contains("unknown field `requires`"));
    }
}
