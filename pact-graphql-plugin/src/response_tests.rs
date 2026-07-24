use super::*;
use crate::schema_index::SchemaIndex;
use serde_json::json;

fn validate_response_json(
    sdl: &str,
    query: &str,
    operation_name: Option<&str>,
    response: &Value,
) -> anyhow::Result<Vec<ResponseMismatch>> {
    let index = SchemaIndex::from_sdl(sdl)?;
    validate_response(&index, query, operation_name, response)
}

const SDL: &str = r#"
type Query { product(id: ID!): Product, products: [Product!]! }
type Product {
  id: ID!
  name: String
  status: ProductStatus!
  rating: Int
  category: Category
}
type Category { id: ID!, name: String! }
enum ProductStatus { ACTIVE ARCHIVED }
"#;

const QUERY: &str = "query GetProduct { product(id: \"10\") { id name status } }";

#[test]
fn accepts_a_well_formed_response() {
    let mismatches = validate_response_json(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({"data": {"product": {"id": "10", "name": "Backpack", "status": "ACTIVE"}}}),
    )
    .unwrap();
    assert!(mismatches.is_empty(), "got {mismatches:#?}");
}

#[test]
fn rejects_a_field_absent_from_the_schema() {
    let mismatches = validate_response_json(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({"data": {"product": {
            "id": "10", "name": "Backpack", "status": "ACTIVE", "internalSku": "INT-001"
        }}}),
    )
    .unwrap();
    assert_eq!(mismatches.len(), 1, "got {mismatches:#?}");
    assert_eq!(mismatches[0].path, "$.data.product.internalSku");
    assert!(
        mismatches[0].description.contains("not a field of type \"Product\""),
        "{}",
        mismatches[0].description
    );
}

#[test]
fn rejects_a_schema_field_the_query_did_not_request() {
    let mismatches = validate_response_json(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({"data": {"product": {
            "id": "10", "name": "Backpack", "status": "ACTIVE", "rating": 5
        }}}),
    )
    .unwrap();
    assert_eq!(mismatches.len(), 1, "got {mismatches:#?}");
    assert_eq!(mismatches[0].path, "$.data.product.rating");
    assert!(mismatches[0].description.contains("was not requested"), "{}", mismatches[0].description);
}

#[test]
fn rejects_a_missing_requested_field() {
    let mismatches = validate_response_json(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({"data": {"product": {"id": "10", "name": "Backpack"}}}),
    )
    .unwrap();
    assert_eq!(mismatches.len(), 1, "got {mismatches:#?}");
    assert_eq!(mismatches[0].path, "$.data.product.status");
    assert!(mismatches[0].description.contains("missing"), "{}", mismatches[0].description);
}

#[test]
fn rejects_an_invalid_enum_member() {
    let mismatches = validate_response_json(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({"data": {"product": {"id": "10", "name": "Backpack", "status": "SOLD_OUT"}}}),
    )
    .unwrap();
    assert_eq!(mismatches.len(), 1, "got {mismatches:#?}");
    assert_eq!(mismatches[0].path, "$.data.product.status");
    assert!(mismatches[0].expected.contains("ACTIVE"));
    assert_eq!(mismatches[0].actual, "SOLD_OUT");
}

#[test]
fn rejects_null_in_a_non_null_field() {
    let mismatches = validate_response_json(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({"data": {"product": {"id": "10", "name": "Backpack", "status": null}}}),
    )
    .unwrap();
    assert_eq!(mismatches.len(), 1, "got {mismatches:#?}");
    assert_eq!(mismatches[0].path, "$.data.product.status");
    assert!(mismatches[0].description.contains("non-null"), "{}", mismatches[0].description);
}

#[test]
fn accepts_null_in_a_nullable_field() {
    let mismatches = validate_response_json(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({"data": {"product": {"id": "10", "name": null, "status": "ACTIVE"}}}),
    )
    .unwrap();
    assert!(mismatches.is_empty(), "got {mismatches:#?}");
}

#[test]
fn rejects_a_scalar_of_the_wrong_json_type() {
    let query = "query Q { product(id: \"10\") { id rating } }";
    let mismatches = validate_response_json(
        SDL,
        query,
        Some("Q"),
        &json!({"data": {"product": {"id": "10", "rating": "five"}}}),
    )
    .unwrap();
    assert_eq!(mismatches.len(), 1, "got {mismatches:#?}");
    assert_eq!(mismatches[0].path, "$.data.product.rating");
    assert!(mismatches[0].description.contains("Int"), "{}", mismatches[0].description);
}

#[test]
fn walks_into_lists() {
    let query = "query Q { products { id category { name } } }";
    let mismatches = validate_response_json(
        SDL,
        query,
        Some("Q"),
        &json!({"data": {"products": [
            {"id": "1", "category": {"name": "Bags"}},
            {"id": "2", "category": {"name": "Bags", "slug": "bags"}}
        ]}}),
    )
    .unwrap();
    assert_eq!(mismatches.len(), 1, "got {mismatches:#?}");
    assert_eq!(mismatches[0].path, "$.data.products[1].category.slug");
}

#[test]
fn rejects_an_object_where_a_list_is_expected() {
    let query = "query Q { products { id } }";
    let mismatches = validate_response_json(
        SDL,
        query,
        Some("Q"),
        &json!({"data": {"products": {"id": "1"}}}),
    )
    .unwrap();
    assert_eq!(mismatches.len(), 1, "got {mismatches:#?}");
    assert_eq!(mismatches[0].path, "$.data.products");
    assert!(mismatches[0].description.contains("list"), "{}", mismatches[0].description);
}

#[test]
fn accepts_an_errors_envelope_with_null_data() {
    let mismatches = validate_response_json(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({
            "data": {"product": null},
            "errors": [{"message": "Product not found", "path": ["product"]}]
        }),
    )
    .unwrap();
    assert!(mismatches.is_empty(), "got {mismatches:#?}");
}

#[test]
fn rejects_an_error_without_a_message() {
    let mismatches = validate_response_json(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({"data": null, "errors": [{"code": "NOT_FOUND"}]}),
    )
    .unwrap();
    assert_eq!(mismatches.len(), 1, "got {mismatches:#?}");
    assert_eq!(mismatches[0].path, "$.errors[0].message");
}

#[test]
fn rejects_an_envelope_with_neither_data_nor_errors() {
    let mismatches =
        validate_response_json(SDL, QUERY, Some("GetProduct"), &json!({"meta": {}})).unwrap();
    assert!(
        mismatches.iter().any(|m| m.path == "$"),
        "got {mismatches:#?}"
    );
}

#[test]
fn rejects_an_unknown_top_level_key() {
    let mismatches = validate_response_json(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({
            "data": {"product": {"id": "10", "name": "Backpack", "status": "ACTIVE"}},
            "meta": {"traceId": "abc"}
        }),
    )
    .unwrap();
    assert_eq!(mismatches.len(), 1, "got {mismatches:#?}");
    assert_eq!(mismatches[0].path, "$.meta");
}
