use std::fs;
use std::path::Path;

use crate::error::CatalogError;
use crate::render::RenderedCatalog;

const JSON_PATH: &str = "catalog/index.json";
const MARKDOWN_PATH: &str = "catalog/index.md";

pub(crate) fn write_catalog(root: &Path, rendered: &RenderedCatalog) -> Result<(), CatalogError> {
    write_file(root, JSON_PATH, &rendered.json)?;
    write_file(root, MARKDOWN_PATH, &rendered.markdown)?;
    Ok(())
}

pub(crate) fn check_catalog(root: &Path, rendered: &RenderedCatalog) -> Result<(), CatalogError> {
    let committed_json = read_file(root, JSON_PATH)?;
    let committed_markdown = read_file(root, MARKDOWN_PATH)?;
    let mut stale = Vec::new();
    if committed_json != rendered.json {
        stale.push(JSON_PATH);
    }
    if committed_markdown != rendered.markdown {
        stale.push(MARKDOWN_PATH);
    }
    if stale.is_empty() {
        Ok(())
    } else {
        Err(CatalogError::new(format!(
            "committed catalog output is stale: {}",
            stale.join(", ")
        )))
    }
}

fn read_file(root: &Path, relative: &str) -> Result<String, CatalogError> {
    fs::read_to_string(root.join(relative))
        .map_err(|error| CatalogError::new(format!("failed to read {relative}: {error}")))
}

fn write_file(root: &Path, relative: &str, content: &str) -> Result<(), CatalogError> {
    fs::write(root.join(relative), content)
        .map_err(|error| CatalogError::new(format!("failed to write {relative}: {error}")))
}
