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
