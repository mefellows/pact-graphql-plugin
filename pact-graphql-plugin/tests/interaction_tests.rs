use std::sync::Arc;

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use pact_graphql_plugin::encoder::Transport;
use pact_graphql_plugin::{GraphqlInteractionBuilder, GraphqlPluginRequest, SchemaRegistry};

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
    assert_eq!(config.variables_json, req.variables_json);
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

    let inline_base64 = config
        .schema_inline_base64
        .expect("inline schema should be embedded");
    let decoded = BASE64_STANDARD.decode(inline_base64).unwrap();
    let decoded_str = String::from_utf8(decoded).unwrap();
    assert_eq!(decoded_str, format!("{}\n", schema_sdl.trim()));
}
