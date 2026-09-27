use std::fs;
use std::path::Path;

use crate::error::CatalogError;
use crate::model::{CatalogIndex, SCHEMA_VERSION};
use crate::query::Catalog;

const JSON_PATH: &str = "catalog/index.json";

pub fn read_catalog_from(root: &Path) -> Result<Catalog, CatalogError> {
    let content = fs::read_to_string(root.join(JSON_PATH))
        .map_err(|error| CatalogError::new(format!("failed to read {JSON_PATH}: {error}")))?;
    read_catalog_str(&content)
}

pub(crate) fn read_catalog_str(content: &str) -> Result<Catalog, CatalogError> {
    let index: CatalogIndex = serde_json::from_str(content)
        .map_err(|error| CatalogError::new(format!("invalid JSON in {JSON_PATH}: {error}")))?;
    if index.schema_version != SCHEMA_VERSION {
        return Err(CatalogError::new(format!(
            "unsupported Catalog schema version: {}",
            index.schema_version
        )));
    }
    Catalog::new(index)
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
    },
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
    }
  ]
}"#
    }

    #[test]
    fn valid_catalog_v1_loads() {
        let catalog = read_catalog_str(valid_catalog()).expect("catalog");
        assert_eq!(catalog.artifacts().len(), 2);
    }

    #[test]
    fn unsupported_schema_version_rejects() {
        let document = valid_catalog().replace("\"schemaVersion\": 1", "\"schemaVersion\": 2");
        assert_eq!(
            read_catalog_str(&document).unwrap_err().to_string(),
            "unsupported Catalog schema version: 2"
        );
    }

    #[test]
    fn invalid_json_is_bounded() {
        assert!(read_catalog_str("{")
            .unwrap_err()
            .to_string()
            .contains("invalid JSON"));
    }
}
