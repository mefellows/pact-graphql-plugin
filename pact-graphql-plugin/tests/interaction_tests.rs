use std::sync::Arc;

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use pact_graphql_plugin::encoder::Transport;
use pact_graphql_plugin::{GraphqlInteractionBuilder, GraphqlPluginRequest, SchemaRegistry};
use serde_json::json;

fn build_builder() -> (tempfile::TempDir, GraphqlInteractionBuilder) {
    let dir = tempfile::tempdir().unwrap();
    let registry = SchemaRegistry::new(dir.path()).unwrap();
    let builder = GraphqlInteractionBuilder::new(Arc::new(registry));
    (dir, builder)
}

#[test]
fn rejects_empty_query() {
    let (_dir, builder) = build_builder();
    let err = builder
        .build(GraphqlPluginRequest {
            query_document: "   ".into(),
            ..Default::default()
        })
        .unwrap_err();

    assert!(err.to_string().contains("query_document"));
}

#[test]
fn propagates_optional_fields_and_transport() {
    let (_dir, builder) = build_builder();
    let req = GraphqlPluginRequest {
        query_document: "query PingQuery { ping }".into(),
        operation_name: Some("PingQuery".into()),
        variables_json: Some(r#"{"id": 1}"#.into()),
        transport: Transport::QueryString,
        schema_sdl: None,
        ..Default::default()
    };

    let config = builder.build(req.clone()).unwrap();

    // The stored document is the canonical form, not the raw input.
    assert_eq!(config.query_document, "query PingQuery {\n  ping\n}");
    assert_eq!(config.operation_name, req.operation_name);
    assert_eq!(config.variables_json.as_deref(), Some("{\"id\":1}"));
    assert_eq!(config.transport, req.transport);
    assert!(config.schema_ref.is_none());
    assert!(config.schema_inline_base64.is_none());
}

#[test]
fn stores_schema_and_embeds_inline_copy() {
    let (_dir, builder) = build_builder();
    let schema_sdl = "type Query { ping: String }";

    let config = builder
        .build(GraphqlPluginRequest {
            query_document: "query { ping }".into(),
            schema_sdl: Some(schema_sdl.into()),
            ..Default::default()
        })
        .unwrap();

    let schema_ref = config.schema_ref.expect("schema_ref should be set");
    assert_eq!(schema_ref.encoding, "utf-8");
}

#[test]
fn canonicalizes_request_payload() {
    let (_dir, builder) = build_builder();
    let schema_sdl = r#"
        type Query {
            product(id: ID!): Product
        }

        type Product {
            id: ID!
            name: String!
        }
    "#;

    let config = builder
        .build(GraphqlPluginRequest {
            query_document: r#"
                query GetProduct($id: ID!) {
                    product(id: $id) {
                        id
        
                        name
                    }
                }
            "#
            .into(),
            // Deliberately out of key order, and carrying an extra variable the operation does
            // not declare (which servers ignore), to pin canonicalisation behaviour.
            variables_json: Some(r#"{ "b": 2, "id": "10", "a": 1 }"#.into()),
            operation_name: Some("GetProduct".into()),
            schema_sdl: Some(schema_sdl.into()),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(
        config.request.query_document,
        "query GetProduct($id: ID!) {\n  product(id: $id) {\n    id\n    name\n  }\n}"
    );
    assert_eq!(config.request.operation_name.as_deref(), Some("GetProduct"));
    let canonical_variables = config
        .request
        .variables_json
        .as_ref()
        .expect("canonical variables should be present");
    let variables_value: serde_json::Value = serde_json::from_str(canonical_variables).unwrap();
    assert_eq!(variables_value, json!({ "a": 1, "b": 2, "id": "10" }));
    assert_eq!(
        canonical_variables.as_str(),
        r#"{"a":1,"b":2,"id":"10"}"#,
        "object keys should be canonically sorted"
    );

    let inline_schema_base64 = config
        .schema_inline_base64
        .as_ref()
        .expect("inline schema should be present");
    let inline_schema_struct = config
        .inline_schema
        .as_ref()
        .expect("inline schema metadata should be present");
    assert_eq!(
        inline_schema_struct.base64_sdl.as_deref(),
        Some(inline_schema_base64.as_str())
    );

    let decoded = String::from_utf8(BASE64_STANDARD.decode(inline_schema_base64).unwrap()).unwrap();
    assert_eq!(
        decoded,
        "type Query {\n    product(id: ID!): Product\n}\n\ntype Product {\n    id: ID!\n    name: String!\n}\n"
    );
}

#[test]
fn fails_when_query_references_unknown_field_in_payload_validation() {
    let (_dir, builder) = build_builder();
    let err = builder
        .build(GraphqlPluginRequest {
            query_document: r#"
                query GetProduct($id: ID!) {
                    product(id: $id) {
                        id
                        status
                    }
                }
            "#
            .into(),
            schema_sdl: Some(
                r#"
                type Query {
                    product(id: ID!): Product
                }

                type Product {
                    id: ID!
                }
                "#
                .into(),
            ),
            ..Default::default()
        })
        .unwrap_err();

    let err_str = err.to_string();
    assert!(
        err_str.contains("GraphQL query validation failed"),
        "unexpected top-level error: {}",
        err_str
    );
    let chain: Vec<String> = err.chain().skip(1).map(|cause| cause.to_string()).collect();
    assert!(
        chain
            .iter()
            .any(|msg| msg.contains("field `status` does not exist on type `Product`")),
        "missing detailed validation error. chain: {:?}",
        chain
    );
}

#[test]
fn fails_when_nested_selection_has_unknown_field() {
    let (_dir, builder) = build_builder();
    let err = builder
        .build(GraphqlPluginRequest {
            query_document: r#"
                query GetProduct($id: ID!) {
                    product(id: $id) {
                        id
                        details {
                            name
                            stock
                        }
                    }
                }
            "#
            .into(),
            schema_sdl: Some(
                r#"
                type Query {
                    product(id: ID!): Product
                }

                type Product {
                    id: ID!
                    details: ProductDetails
                }

                type ProductDetails {
                    name: String!
                }
                "#
                .into(),
            ),
            ..Default::default()
        })
        .unwrap_err();

    let err_str = err.to_string();
    assert!(
        err_str.contains("GraphQL query validation failed"),
        "unexpected top-level error: {}",
        err_str
    );
    let chain: Vec<String> = err.chain().skip(1).map(|cause| cause.to_string()).collect();
    assert!(
        chain
            .iter()
            .any(|msg| msg.contains("field `stock` does not exist on type `ProductDetails`")),
        "missing detailed validation error. chain: {:?}",
        chain
    );
}

#[test]
fn fails_when_introspection_type_missing_required_argument() {
    let (_dir, builder) = build_builder();
    let err = builder
        .build(GraphqlPluginRequest {
            query_document: "query { __type { name } }".into(),
            schema_sdl: Some(
                r#"
                type Query {
                    product(id: ID!): Product
                }

                type Product {
                    id: ID!
                }
                "#
                .into(),
            ),
            ..Default::default()
        })
        .unwrap_err();

    let err_str = err.to_string();
    assert!(
        err_str.contains("GraphQL query validation failed"),
        "unexpected top-level error: {}",
        err_str
    );
    let chain: Vec<String> = err.chain().skip(1).map(|cause| cause.to_string()).collect();
    assert!(
        chain.iter().any(|msg| msg
            .contains("field `__type` on type `Query` is missing required argument `name`")),
        "missing detailed validation error. chain: {:?}",
        chain
    );
}

#[test]
fn fails_when_introspection_type_has_unknown_argument() {
    let (_dir, builder) = build_builder();
    let err = builder
        .build(GraphqlPluginRequest {
            query_document: "query { __type(name: \"Product\", foo: \"bar\") { name } }".into(),
            schema_sdl: Some(
                r#"
                type Query {
                    product(id: ID!): Product
                }

                type Product {
                    id: ID!
                }
                "#
                .into(),
            ),
            ..Default::default()
        })
        .unwrap_err();

    let err_str = err.to_string();
    assert!(
        err_str.contains("GraphQL query validation failed"),
        "unexpected top-level error: {}",
        err_str
    );
    let chain: Vec<String> = err.chain().skip(1).map(|cause| cause.to_string()).collect();
    assert!(
        chain
            .iter()
            .any(|msg| msg.contains("argument `foo` is not defined on field `__type`")),
        "missing detailed validation error. chain: {:?}",
        chain
    );
}

#[test]
fn fails_when_fragments_form_cycle() {
    let (_dir, builder) = build_builder();
    let err = builder
        .build(GraphqlPluginRequest {
            query_document: r#"
                query GetProduct($id: ID!) {
                    product(id: $id) {
                        ...NodeFields
                    }
                }

                fragment NodeFields on Product {
                    id
                    ...OtherFields
                }

                fragment OtherFields on Product {
                    name
                    ...NodeFields
                }
            "#
            .into(),
            schema_sdl: Some(
                r#"
                type Query {
                    product(id: ID!): Product
                }

                type Product {
                    id: ID!
                    name: String!
                }
                "#
                .into(),
            ),
            ..Default::default()
        })
        .unwrap_err();

    // Canonicalisation inlines fragment spreads, so it reaches the cycle before
    // schema validation does. It reports the cycle in the same form the
    // validation path would, so the diagnostic is identical either way.
    let chain: Vec<String> = err.chain().map(|cause| cause.to_string()).collect();
    assert!(
        chain
            .iter()
            .any(|msg| msg.contains("fragment `NodeFields` forms a cycle")),
        "missing fragment cycle diagnostic. chain: {:?}",
        chain
    );
}

#[test]
fn wire_config_carries_the_schema_exactly_once() {
    // The SDL is the largest thing in a pact file. Carrying it in both `inline_schema.base64_sdl`
    // and the legacy `schema_inline_base64` doubled every interaction's footprint; measured on
    // examples/js/product-consumer that was 8 copies of a 3.8KB blob across 4 interactions.
    let (_dir, builder) = build_builder();
    let sdl = "type Query { hello: String }";

    let config = builder
        .build(GraphqlPluginRequest {
            query_document: "query Q { hello }".into(),
            operation_name: Some("Q".into()),
            schema_sdl: Some(sdl.into()),
            ..GraphqlPluginRequest::default()
        })
        .expect("build config");

    let wire = serde_json::to_value(&config).expect("serialize config");

    assert!(
        wire.get("inline_schema")
            .and_then(|schema| schema.get("base64_sdl"))
            .and_then(|sdl| sdl.as_str())
            .is_some(),
        "inline_schema.base64_sdl is the canonical home for the SDL"
    );
    assert!(
        wire.get("schema_inline_base64").is_none_or(|v| v.is_null()),
        "the legacy duplicate must no longer be written, got: {wire:#}"
    );
}

#[test]
fn wire_config_still_reads_the_legacy_schema_field() {
    // Pacts written before the duplicate was dropped must keep verifying.
    let legacy = json!({
        "query_document": "query Q { hello }",
        "operation_name": "Q",
        "transport": "json_body",
        "schema_inline_base64": BASE64_STANDARD.encode("type Query { hello: String }"),
    });

    let config: pact_graphql_plugin::GraphqlPluginConfig =
        serde_json::from_value(legacy).expect("legacy config should deserialize");

    assert!(
        config.inline_schema.is_some(),
        "the legacy field should still populate the inline schema"
    );
}

#[test]
fn rejects_variables_that_do_not_satisfy_the_operations_declarations() {
    let (_dir, builder) = build_builder();
    let sdl = "schema { query: Query }\ntype Product { id: ID! }\ntype Query { product(id: ID!): Product }";

    let err = builder
        .build(GraphqlPluginRequest {
            query_document: "query GetProduct($id: ID!) { product(id: $id) { id } }".into(),
            operation_name: Some("GetProduct".into()),
            // The author typo'd the variable name, so `$id` is never supplied.
            variables_json: Some(r#"{"productId":"10"}"#.into()),
            schema_sdl: Some(sdl.into()),
            ..GraphqlPluginRequest::default()
        })
        .expect_err("a missing required variable should be rejected at configure time");

    let message = format!("{err:#}");
    assert!(
        message.contains("$id"),
        "error should name the variable, got: {message}"
    );
}

#[test]
fn accepts_variables_that_satisfy_the_operations_declarations() {
    let (_dir, builder) = build_builder();
    let sdl = "schema { query: Query }\ntype Product { id: ID! }\ntype Query { product(id: ID!): Product }";

    builder
        .build(GraphqlPluginRequest {
            query_document: "query GetProduct($id: ID!) { product(id: $id) { id } }".into(),
            operation_name: Some("GetProduct".into()),
            variables_json: Some(r#"{"id":"10"}"#.into()),
            schema_sdl: Some(sdl.into()),
            ..GraphqlPluginRequest::default()
        })
        .expect("well-formed variables should be accepted");
}

#[test]
fn wire_config_omits_absent_fields_rather_than_writing_nulls() {
    // Null keys are noise in every interaction of every pact file. They carry no information the
    // absent key does not, and `serde` deserialises a missing `Option` field to `None` anyway.
    let (_dir, builder) = build_builder();

    let config = builder
        .build(GraphqlPluginRequest {
            query_document: "query Q { hello }".into(),
            ..GraphqlPluginRequest::default()
        })
        .expect("build config");

    let wire = serde_json::to_value(&config).expect("serialize config");
    let object = wire.as_object().expect("config serialises to an object");

    let nulls: Vec<&String> = object
        .iter()
        .filter(|(_, value)| value.is_null())
        .map(|(key, _)| key)
        .collect();

    assert!(nulls.is_empty(), "expected no null keys, found: {nulls:?}");
}
