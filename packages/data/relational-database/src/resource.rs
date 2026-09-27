use crate::{RelationalDatabaseError, RelationalQueryResult, RelationalValue};

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
