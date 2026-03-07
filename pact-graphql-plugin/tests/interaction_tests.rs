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
        query_document: "query { ping }".into(),
        operation_name: Some("PingQuery".into()),
        variables_json: Some(r#"{"id": 1}"#.into()),
        transport: Transport::QueryString,
        schema_sdl: None,
    };

    let config = builder.build(req.clone()).unwrap();

    assert_eq!(config.query_document, req.query_document);
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
            variables_json: Some(r#"{ "b": 2, "a": 1 }"#.into()),
            operation_name: Some("GetProduct".into()),
            schema_sdl: Some(schema_sdl.into()),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(
        config.request.query_document,
        "query GetProduct($id: ID!) {\n    product(id: $id) {\n        id\n\n        name\n    }\n}"
    );
    assert_eq!(config.request.operation_name.as_deref(), Some("GetProduct"));
    let canonical_variables = config
        .request
        .variables_json
        .as_ref()
        .expect("canonical variables should be present");
    let variables_value: serde_json::Value = serde_json::from_str(canonical_variables).unwrap();
    assert_eq!(variables_value, json!({ "a": 1, "b": 2 }));

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
            .any(|msg| msg.contains("fragment `NodeFields` forms a cycle")),
        "missing detailed validation error. chain: {:?}",
        chain
    );
}
