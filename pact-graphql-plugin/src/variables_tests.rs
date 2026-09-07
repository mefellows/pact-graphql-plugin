use super::*;
use crate::query_ast::parse_and_inline;
use serde_json::json;

const SDL: &str = r#"
schema { query: Query }
enum Status { ACTIVE DRAFT }
input Filter { term: String }
type Product { id: ID! }
type Query {
  product(id: ID!): Product
  products(status: Status, first: Int, tags: [String!], filter: Filter, rate: Float, live: Boolean): [Product!]!
}
"#;

fn check(query: &str, variables: Value) -> anyhow::Result<()> {
    let schema = SchemaIndex::from_sdl(SDL).expect("schema parses");
    let operation = parse_and_inline(query, None).expect("query parses");
    validate_variables(&schema, &operation, Some(&variables))
}

#[test]
fn accepts_a_supplied_required_variable() {
    let result = check(
        "query Q($id: ID!) { product(id: $id) { id } }",
        json!({ "id": "10" }),
    );
    assert!(result.is_ok(), "expected ok, got: {result:?}");
}

#[test]
fn rejects_a_missing_required_variable() {
    let err = check("query Q($id: ID!) { product(id: $id) { id } }", json!({}))
        .expect_err("missing required variable should fail");

    let message = format!("{err:#}");
    assert!(
        message.contains("$id"),
        "error should name the variable, got: {message}"
    );
}

#[test]
fn rejects_a_required_variable_supplied_as_null() {
    let err = check(
        "query Q($id: ID!) { product(id: $id) { id } }",
        json!({ "id": null }),
    )
    .expect_err("explicit null for a non-null variable should fail");

    assert!(format!("{err:#}").contains("$id"));
}

#[test]
fn accepts_a_missing_optional_variable() {
    let result = check(
        "query Q($status: Status) { products(status: $status) { id } }",
        json!({}),
    );
    assert!(result.is_ok(), "expected ok, got: {result:?}");
}

#[test]
fn accepts_a_missing_non_null_variable_that_has_a_default() {
    let result = check(
        "query Q($first: Int! = 10) { products(first: $first) { id } }",
        json!({}),
    );
    assert!(result.is_ok(), "expected ok, got: {result:?}");
}

#[test]
fn rejects_a_string_supplied_for_an_int_variable() {
    let err = check(
        "query Q($first: Int) { products(first: $first) { id } }",
        json!({ "first": "ten" }),
    )
    .expect_err("a string is not an Int");

    let message = format!("{err:#}");
    assert!(message.contains("$first"), "got: {message}");
    assert!(message.contains("Int"), "got: {message}");
}

#[test]
fn rejects_a_fractional_value_for_an_int_variable() {
    let err = check(
        "query Q($first: Int) { products(first: $first) { id } }",
        json!({ "first": 1.5 }),
    )
    .expect_err("1.5 is not an Int");
    assert!(format!("{err:#}").contains("$first"));
}

#[test]
fn accepts_an_integer_for_a_float_variable() {
    let result = check(
        "query Q($rate: Float) { products(rate: $rate) { id } }",
        json!({ "rate": 3 }),
    );
    assert!(result.is_ok(), "Int is valid input coercion for Float: {result:?}");
}

#[test]
fn accepts_a_number_for_an_id_variable() {
    let result = check(
        "query Q($id: ID!) { product(id: $id) { id } }",
        json!({ "id": 10 }),
    );
    assert!(result.is_ok(), "ID accepts Int or String: {result:?}");
}

#[test]
fn rejects_a_value_outside_the_enum() {
    let err = check(
        "query Q($status: Status) { products(status: $status) { id } }",
        json!({ "status": "SOLD_OUT" }),
    )
    .expect_err("SOLD_OUT is not a Status");

    let message = format!("{err:#}");
    assert!(message.contains("$status"), "got: {message}");
    assert!(message.contains("SOLD_OUT"), "got: {message}");
}

#[test]
fn accepts_a_valid_enum_value() {
    let result = check(
        "query Q($status: Status) { products(status: $status) { id } }",
        json!({ "status": "ACTIVE" }),
    );
    assert!(result.is_ok(), "expected ok, got: {result:?}");
}

#[test]
fn rejects_a_scalar_supplied_for_a_list_variable() {
    let err = check(
        "query Q($tags: [String!]) { products(tags: $tags) { id } }",
        json!({ "tags": "sale" }),
    )
    .expect_err("a bare string is not a list");
    assert!(format!("{err:#}").contains("$tags"));
}

#[test]
fn rejects_a_null_item_in_a_non_null_item_list() {
    let err = check(
        "query Q($tags: [String!]) { products(tags: $tags) { id } }",
        json!({ "tags": ["sale", null] }),
    )
    .expect_err("null is not allowed as a [String!] item");
    assert!(format!("{err:#}").contains("$tags"));
}

#[test]
fn accepts_a_well_formed_list() {
    let result = check(
        "query Q($tags: [String!]) { products(tags: $tags) { id } }",
        json!({ "tags": ["sale", "new"] }),
    );
    assert!(result.is_ok(), "expected ok, got: {result:?}");
}

#[test]
fn rejects_a_scalar_supplied_for_an_input_object_variable() {
    let err = check(
        "query Q($filter: Filter) { products(filter: $filter) { id } }",
        json!({ "filter": "term" }),
    )
    .expect_err("an input object variable needs an object");
    assert!(format!("{err:#}").contains("$filter"));
}

#[test]
fn ignores_extra_variables_the_operation_does_not_declare() {
    // Servers ignore undeclared variables, so this is not an error. Asserted so the permissive
    // choice is deliberate rather than accidental.
    let result = check(
        "query Q($id: ID!) { product(id: $id) { id } }",
        json!({ "id": "10", "unused": true }),
    );
    assert!(result.is_ok(), "expected ok, got: {result:?}");
}

#[test]
fn reports_an_unknown_variable_type() {
    let err = check(
        "query Q($id: Nonexistent!) { product(id: $id) { id } }",
        json!({ "id": "10" }),
    )
    .expect_err("a variable typed by a type not in the schema should fail");
    assert!(format!("{err:#}").contains("Nonexistent"));
}

#[test]
fn accepts_a_boolean_variable() {
    let result = check(
        "query Q($live: Boolean) { products(live: $live) { id } }",
        json!({ "live": true }),
    );
    assert!(result.is_ok(), "expected ok, got: {result:?}");
}

#[test]
fn treats_absent_variables_as_an_empty_set() {
    let schema = SchemaIndex::from_sdl(SDL).expect("schema parses");
    let operation =
        parse_and_inline("query Q($id: ID!) { product(id: $id) { id } }", None).expect("parses");

    let err = validate_variables(&schema, &operation, None)
        .expect_err("no variables at all still misses a required one");
    assert!(format!("{err:#}").contains("$id"));
}
