use fabric_package_relational_database::{RelationalQueryResult, RelationalRow, RelationalValue};

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
