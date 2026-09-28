use serde::{Deserialize, Serialize};

pub(crate) const CATALOG_V1: u8 = 1;
pub(crate) const CATALOG_V2: u8 = 2;
pub(crate) const CURRENT_SCHEMA_VERSION: u8 = CATALOG_V2;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogIndex {
    pub(crate) schema_version: u8,
    pub(crate) artifacts: Vec<CatalogArtifact>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogArtifact {
    pub kind: ArtifactKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    pub path: String,
    pub cargo_package: String,
    pub version: String,
    pub description: String,
    pub publishable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readme: Option<String>,
    pub dependencies: CatalogDependencies,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CatalogDependencies {
    pub normal: Vec<String>,
    pub development: Vec<String>,
    pub build: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactKind {
    Package,
    Host,
    Composition,
    Instance,
    Example,
}

impl ArtifactKind {
    pub(crate) fn heading(&self) -> &'static str {
        match self {
            Self::Package => "Packages",
            Self::Host => "Hosts",
            Self::Composition => "Compositions",
            Self::Instance => "Instances",
            Self::Example => "Examples",
        }
    }
}
