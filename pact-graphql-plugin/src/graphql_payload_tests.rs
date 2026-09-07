use super::*;

#[test]
fn canonicalize_query_falls_back_to_dedented_text_on_syntax_error() {
    let input = "query Q { unclosed";
    let result = canonicalize_query(input, Some("Q")).unwrap();
    assert_eq!(result, dedent_and_trim(input));
}

#[test]
fn canonicalize_variables_is_insensitive_to_key_order() {
    // Two JSON objects with the same entries in different orders are the same GraphQL
    // variables, and must not read as a Pact mismatch. Note `serde_json` is built with
    // `preserve_order` here, so a plain round-trip through `Value` does NOT sort keys.
    let a = canonicalize_variables(Some(r#"{"a":1,"b":2}"#.to_string()))
        .unwrap()
        .unwrap();
    let b = canonicalize_variables(Some(r#"{"b":2,"a":1}"#.to_string()))
        .unwrap()
        .unwrap();

    assert_eq!(a, b);
}

#[test]
fn canonicalize_variables_sorts_nested_object_keys() {
    let a = canonicalize_variables(Some(r#"{"outer":{"x":1,"y":[{"p":1,"q":2}]}}"#.to_string()))
        .unwrap()
        .unwrap();
    let b = canonicalize_variables(Some(r#"{"outer":{"y":[{"q":2,"p":1}],"x":1}}"#.to_string()))
        .unwrap()
        .unwrap();

    assert_eq!(a, b);
}

#[test]
fn canonicalize_variables_preserves_array_order() {
    // Array order is semantically meaningful in GraphQL variables; only object keys are sorted.
    let canonical = canonicalize_variables(Some(r#"{"ids":[3,1,2]}"#.to_string()))
        .unwrap()
        .unwrap();

    assert_eq!(canonical, r#"{"ids":[3,1,2]}"#);
}

const ARG_SDL: &str = r#"
schema { query: Query }
enum ProductStatus { ACTIVE DRAFT }
input Filter { term: String }
type Product { id: ID! name: String! }
type Query {
  product(id: ID!): Product
  products(status: ProductStatus, first: Int, rate: Float, live: Boolean, filter: Filter, tags: [String!]): [Product!]!
}
"#;

fn validate_arg_query(query: &str) -> anyhow::Result<()> {
    let schema = SchemaIndex::from_sdl(ARG_SDL).expect("schema parses");
    validate_query_document(&schema, query, None)
}

#[test]
fn rejects_an_enum_argument_value_outside_the_enum() {
    let err = validate_arg_query("query Q { products(status: DISCONTINUED) { id } }")
        .expect_err("DISCONTINUED is not a ProductStatus");

    let message = format!("{err:#}");
    assert!(message.contains("DISCONTINUED"), "got: {message}");
    assert!(message.contains("ProductStatus"), "got: {message}");
}

#[test]
fn accepts_a_valid_enum_argument_value() {
    validate_arg_query("query Q { products(status: ACTIVE) { id } }").expect("ACTIVE is valid");
}

#[test]
fn rejects_a_string_literal_for_an_int_argument() {
    let err = validate_arg_query(r#"query Q { products(first: "ten") { id } }"#)
        .expect_err("a string is not an Int");
    assert!(format!("{err:#}").contains("first"));
}

#[test]
fn accepts_an_integer_literal_for_a_float_argument() {
    validate_arg_query("query Q { products(rate: 3) { id } }")
        .expect("Int coerces to Float");
}

#[test]
fn accepts_a_variable_reference_as_an_argument_value() {
    // The variable's own value is checked against its declaration separately; an argument that
    // refers to a variable must not be rejected here.
    validate_arg_query("query Q($status: ProductStatus) { products(status: $status) { id } }")
        .expect("a variable reference is a valid argument value");
}

#[test]
fn rejects_null_for_a_non_null_argument() {
    let err = validate_arg_query("query Q { product(id: null) { id } }")
        .expect_err("id is ID!");
    assert!(format!("{err:#}").contains("id"));
}

#[test]
fn accepts_a_well_formed_list_argument() {
    validate_arg_query(r#"query Q { products(tags: ["a", "b"]) { id } }"#)
        .expect("a list of strings is valid for [String!]");
}

#[test]
fn rejects_a_scalar_for_a_list_argument() {
    let err = validate_arg_query(r#"query Q { products(tags: "a") { id } }"#)
        .expect_err("a bare string is not a list");
    assert!(format!("{err:#}").contains("tags"));
}
