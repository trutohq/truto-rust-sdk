//! Runs every case in `tests/fixtures/qs_cases.json` (generated with
//! `qs.stringify(obj, { sort })`) through the query encoder.

use serde_json::Value;

#[test]
fn qs_fixture_cases() {
    let raw = include_str!("fixtures/qs_cases.json");
    let cases: Vec<Value> = serde_json::from_str(raw).expect("fixture parses");
    assert!(!cases.is_empty());
    for case in &cases {
        let name = case["name"].as_str().unwrap();
        let input = case["input"].as_object().expect("input is an object");
        let expected = case["expected"].as_str().unwrap();
        assert_eq!(truto::encode_query(input), expected, "case {name:?}");
    }
}

#[test]
fn query_builder_matches_json_input() {
    let q = truto::Query::new()
        .set("limit", 10)
        .set("integrated_account_id", "ia_1");
    assert_eq!(q.encode(), "integrated_account_id=ia_1&limit=10");
}
