use pact_graphql_plugin::test_support::schema_index_for;

const SDL: &str = r#"
type Query { product(id: ID!): Product }
type Product { id: ID!, name: String, status: ProductStatus!, tags: [String!]! }
enum ProductStatus { ACTIVE ARCHIVED }
"#;

#[test]
fn exposes_enum_values() {
    let index = schema_index_for(SDL).expect("schema parses");
    let values = index.enum_values("ProductStatus").expect("enum is known");
    assert!(values.contains("ACTIVE"));
    assert!(values.contains("ARCHIVED"));
    assert_eq!(values.len(), 2);
}

#[test]
fn unknown_enum_returns_none() {
    let index = schema_index_for(SDL).expect("schema parses");
    assert!(index.enum_values("Product").is_none());
    assert!(index.enum_values("Nope").is_none());
}

#[test]
fn resolves_field_return_types() {
    let index = schema_index_for(SDL).expect("schema parses");

    let id = index.field_return_type("Product", "id").expect("id exists");
    assert!(id.is_non_null());
    assert_eq!(id.innermost_named(), Some("ID"));

    let name = index.field_return_type("Product", "name").expect("name exists");
    assert!(!name.is_non_null());

    assert!(index.field_return_type("Product", "nope").is_none());
}

#[test]
fn unwraps_list_and_non_null() {
    let index = schema_index_for(SDL).expect("schema parses");
    let tags = index.field_return_type("Product", "tags").expect("tags exists");

    // [String!]! -> NonNull(List(NonNull(String)))
    assert!(tags.is_non_null());
    let list = tags.unwrap_non_null();
    let item = list.as_list_item().expect("is a list");
    assert!(item.is_non_null());
    assert_eq!(item.unwrap_non_null().innermost_named(), Some("String"));
}

#[test]
fn classifies_enums_and_scalars() {
    let index = schema_index_for(SDL).expect("schema parses");
    assert!(index.is_enum("ProductStatus"));
    assert!(!index.is_enum("Product"));
    assert!(index.is_scalar("ID"));
    assert!(index.is_scalar("String"));
    assert!(!index.is_scalar("Product"));
}
