//! RelationalDatabase semantic package for Fabric.
//!
//! This package owns the generic relational database Resource and its portable
//! value/result/error types. Concrete backends such as SQLite belong in
//! realization packages.

#[derive(Clone, Debug, PartialEq)]
pub enum RelationalValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Bytes(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RelationalRow {
    values: Vec<RelationalValue>,
}

impl RelationalRow {
    pub fn new(values: Vec<RelationalValue>) -> Self {
        Self { values }
    }

    pub fn values(&self) -> &[RelationalValue] {
        &self.values
    }

    pub fn into_values(self) -> Vec<RelationalValue> {
        self.values
    }

    pub fn get(&self, index: usize) -> Option<&RelationalValue> {
        self.values.get(index)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RelationalQueryResult {
    rows: Vec<RelationalRow>,
}

impl RelationalQueryResult {
    pub fn new(rows: Vec<RelationalRow>) -> Self {
        Self { rows }
    }

    pub fn rows(&self) -> &[RelationalRow] {
        &self.rows
    }

    pub fn into_rows(self) -> Vec<RelationalRow> {
        self.rows
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelationalDatabaseErrorKind {
    NotStarted,
    Stopped,
    OpenFailed,
    ExecuteFailed,
    QueryFailed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelationalDatabaseError {
    pub kind: RelationalDatabaseErrorKind,
    pub detail: String,
}

impl RelationalDatabaseError {
    pub fn new(kind: RelationalDatabaseErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    pub fn not_started() -> Self {
        Self::new(
            RelationalDatabaseErrorKind::NotStarted,
            "relational database has not started",
        )
    }

    pub fn stopped() -> Self {
        Self::new(
            RelationalDatabaseErrorKind::Stopped,
            "relational database generation is stopped",
        )
    }
}

impl std::fmt::Display for RelationalDatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

impl std::error::Error for RelationalDatabaseError {}

fabric::resource! {
    pub RelationalDatabase {
        id: "onoal.package.data.relational-database";

        api {
            fn execute(
                &self,
                statement: String,
                parameters: Vec<RelationalValue>,
            ) -> Result<usize, RelationalDatabaseError>;

            fn query(
                &self,
                statement: String,
                parameters: Vec<RelationalValue>,
            ) -> Result<RelationalQueryResult, RelationalDatabaseError>;
        }
    }
}

pub fn relational_database_requirement() -> fabric::authoring::Requires<RelationalDatabase> {
    fabric::authoring::Requires::<RelationalDatabase>::provisional()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_rows_and_results_are_package_owned_and_inspectable() {
        let row = RelationalRow::new(vec![
            RelationalValue::Null,
            RelationalValue::Integer(7),
            RelationalValue::Real(3.5),
            RelationalValue::Text("hello".to_owned()),
            RelationalValue::Bytes(vec![1, 2, 3]),
        ]);
        assert_eq!(row.get(1), Some(&RelationalValue::Integer(7)));

        let result = RelationalQueryResult::new(vec![row.clone()]);
        assert_eq!(result.rows(), &[row]);
    }
}
