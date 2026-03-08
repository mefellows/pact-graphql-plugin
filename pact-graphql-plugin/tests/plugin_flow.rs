use std::collections::HashMap;

use pact_plugin_driver::proto::generate_content_request::{ContentFor, TestMode};
use pact_plugin_driver::proto::pact_plugin_server::PactPlugin;
use pact_plugin_driver::proto::{ConfigureInteractionRequest, GenerateContentRequest};
use pact_plugin_driver::utils::to_proto_struct;
use tempfile::TempDir;
use tonic::Request;

use pact_graphql_plugin::interaction::GraphqlPluginRequest;
use pact_graphql_plugin::schema::SchemaRegistry;
use pact_graphql_plugin::server::GraphqlPlugin;

fn temp_plugin() -> (TempDir, GraphqlPlugin) {
    let dir = tempfile::tempdir().unwrap();
    let registry = SchemaRegistry::new(dir.path()).unwrap();
    let plugin = GraphqlPlugin::with_registry(registry);
    (dir, plugin)
}

fn make_contents_config(req: &GraphqlPluginRequest) -> ConfigureInteractionRequest {
    let value = serde_json::to_value(req).unwrap();
    let mut map = HashMap::new();
    if let serde_json::Value::Object(obj) = value {
        for (k, v) in obj {
            map.insert(k, v);
        }
    }

    ConfigureInteractionRequest {
        content_type: "application/graphql".to_string(),
        contents_config: Some(to_proto_struct(&map)),
    }
}

fn expected_graphql_body() -> &'static str {
    r#"{"query":"query PingQuery { ping }","operationName":"PingQuery","variables":{"id":1}}"#
}

#[tokio::test]
async fn configure_and_generate_json_body() {
    let (_dir, plugin) = temp_plugin();

    let req = GraphqlPluginRequest {
        query_document: "query PingQuery { ping }".into(),
        operation_name: Some("PingQuery".into()),
        variables_json: Some(r#"{"id":1}"#.into()),
        transport: pact_graphql_plugin::encoder::Transport::JsonBody,
        schema_sdl: Some("type Query { ping: String }".into()),
    };

    let configure_request = make_contents_config(&req);
    let response = plugin
        .configure_interaction(Request::new(configure_request))
        .await
        .unwrap();
    let response = response.get_ref();

    assert!(response.error.is_empty());
    let interaction = response.interaction.first().expect("interaction");
    let body = interaction.contents.as_ref().expect("body");
    assert_eq!(body.content_type, "application/graphql");
    let content = String::from_utf8(body.content.clone().unwrap()).unwrap();
    assert_eq!(content, expected_graphql_body());

    let plugin_config = response
        .plugin_configuration
        .as_ref()
        .expect("plugin configuration");
    assert!(plugin_config.interaction_configuration.is_some());

    let generate_request = GenerateContentRequest {
        contents: None,
        generators: Default::default(),
        plugin_configuration: Some(plugin_config.clone()),
        test_context: None,
        test_mode: TestMode::Consumer as i32,
        content_for: ContentFor::Request as i32,
    };
    let generated = plugin
        .generate_content(Request::new(generate_request))
        .await
        .unwrap();
    let generated_body = generated
        .get_ref()
        .contents
        .as_ref()
        .expect("generated body");
    let generated_content = String::from_utf8(generated_body.content.clone().unwrap()).unwrap();
    assert_eq!(generated_content, expected_graphql_body());
}
