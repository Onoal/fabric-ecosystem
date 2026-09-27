use serde::Serialize;

pub(crate) const SCHEMA_VERSION: u8 = 1;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogIndex {
    pub(crate) schema_version: u8,
    pub(crate) artifacts: Vec<CatalogArtifact>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CatalogArtifact {
    pub(crate) kind: ArtifactKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) category: Option<String>,
    pub(crate) path: String,
    pub(crate) cargo_package: String,
    pub(crate) version: String,
    pub(crate) description: String,
    pub(crate) publishable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) readme: Option<String>,
    pub(crate) dependencies: CatalogDependencies,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub(crate) struct CatalogDependencies {
    pub(crate) normal: Vec<String>,
    pub(crate) development: Vec<String>,
    pub(crate) build: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ArtifactKind {
    Package,
    Host,
    Composition,
    Example,
}

impl ArtifactKind {
    pub(crate) fn heading(&self) -> &'static str {
        match self {
            Self::Package => "Packages",
            Self::Host => "Hosts",
            Self::Composition => "Compositions",
            Self::Example => "Examples",
        }
    }
}
