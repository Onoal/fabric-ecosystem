use crate::{RelationalDatabaseError, RelationalQueryResult, RelationalValue};

fabric::resource! {
    pub RelationalDatabase {
        id: "onoal.package.data.relational-database";

        api {
            async fn execute(
                &self,
                statement: String,
                parameters: Vec<RelationalValue>,
            ) -> Result<usize, RelationalDatabaseError>;

            async fn query(
                &self,
                statement: String,
                parameters: Vec<RelationalValue>,
            ) -> Result<RelationalQueryResult, RelationalDatabaseError>;
        }
    }
}
