use std::collections::HashMap;

use pact_plugin_driver::proto::generate_content_request::{ContentFor, TestMode};
use pact_plugin_driver::proto::pact_plugin_server::PactPlugin;
use pact_plugin_driver::proto::{
    Body, CompareContentsRequest, ConfigureInteractionRequest, GenerateContentRequest,
    InitPluginRequest, PluginConfiguration,
};
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
    make_contents_config_with_type(req, "application/graphql")
}

fn make_contents_config_with_type(
    req: &GraphqlPluginRequest,
    content_type: &str,
) -> ConfigureInteractionRequest {
    let value = serde_json::to_value(req).unwrap();
    let mut map = HashMap::new();
    if let serde_json::Value::Object(obj) = value {
        for (k, v) in obj {
            map.insert(k, v);
        }
    }

    ConfigureInteractionRequest {
        content_type: content_type.to_string(),
        contents_config: Some(to_proto_struct(&map)),
    }
}

fn expected_graphql_body() -> &'static str {
    r#"{"query":"query PingQuery {\n  ping\n}","operationName":"PingQuery","variables":{"id":1}}"#
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
        ..Default::default()
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

const PRODUCT_SDL: &str = r#"
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

const PRODUCT_QUERY: &str = "query GetProduct { product(id: \"10\") { id name status } }";

fn product_request(response_body_json: Option<&str>) -> GraphqlPluginRequest {
    GraphqlPluginRequest {
        query_document: PRODUCT_QUERY.into(),
        operation_name: Some("GetProduct".into()),
        variables_json: None,
        transport: pact_graphql_plugin::encoder::Transport::JsonBody,
        schema_sdl: Some(PRODUCT_SDL.into()),
        query_matching: Default::default(),
        response_body_json: response_body_json.map(str::to_string),
    }
}

#[tokio::test]
async fn advertises_a_response_content_matcher() {
    let (_dir, plugin) = temp_plugin();
    let catalogue = plugin
        .init_plugin(Request::new(InitPluginRequest::default()))
        .await
        .expect("init succeeds")
        .into_inner()
        .catalogue;

    assert!(
        catalogue.iter().any(|entry| entry.key == "graphql-response"
            && entry.values.get("content-types")
                == Some(&"application/graphql-response".to_string())),
        "catalogue is {catalogue:#?}"
    );
}

#[tokio::test]
async fn configure_with_request_content_type_returns_a_single_request_part() {
    let (_dir, plugin) = temp_plugin();
    // response_body_json is set here to prove the request-side call ignores it and still
    // returns exactly one part, regardless of whether a response is also being configured.
    let request = product_request(Some(
        r#"{"data":{"product":{"id":"10","name":"Backpack","status":"ACTIVE"}}}"#,
    ));

    let response = plugin
        .configure_interaction(Request::new(make_contents_config_with_type(
            &request,
            "application/graphql",
        )))
        .await
        .expect("configure succeeds")
        .into_inner();

    assert_eq!(response.error, "");
    assert_eq!(response.interaction.len(), 1, "exactly one part");

    let part = &response.interaction[0];
    assert_eq!(part.part_name, "request");
    assert!(
        part.plugin_configuration.is_some(),
        "request part carries plugin_configuration"
    );
    assert!(
        response.plugin_configuration.is_some(),
        "top-level plugin_configuration is set for the request call"
    );
}

#[tokio::test]
async fn configure_returns_a_response_part_with_derived_rules() {
    let (_dir, plugin) = temp_plugin();
    let request = product_request(Some(
        r#"{"data":{"product":{"id":"10","name":"Backpack","status":"ACTIVE"}}}"#,
    ));

    let response = plugin
        .configure_interaction(Request::new(make_contents_config_with_type(
            &request,
            "application/graphql-response",
        )))
        .await
        .expect("configure succeeds")
        .into_inner();

    assert_eq!(response.error, "");
    assert_eq!(response.interaction.len(), 1, "exactly one response part");

    let response_part = &response.interaction[0];
    assert_eq!(response_part.part_name, "response");

    assert_eq!(
        response_part
            .contents
            .as_ref()
            .map(|body| body.content_type.as_str()),
        Some("application/json")
    );

    // Load-bearing: the response-side call must NOT return plugin_configuration, otherwise it
    // clobbers the interaction's single shared plugin_config slot and destroys the request-side
    // call's variables_json. See server.rs `configure_response` doc comment.
    assert!(
        response_part.plugin_configuration.is_none(),
        "response part must not carry plugin_configuration"
    );
    assert!(
        response.plugin_configuration.is_none(),
        "top-level plugin_configuration must not be set for the response call"
    );

    let status_rule = response_part
        .rules
        .get("$.data.product.status")
        .expect("enum rule is derived");
    assert_eq!(status_rule.rule[0].r#type, "regex");

    let name_rule = response_part
        .rules
        .get("$.data.product.name")
        .expect("scalar rule is derived");
    assert_eq!(name_rule.rule[0].r#type, "type");
}

#[tokio::test]
async fn configure_rejects_a_response_that_violates_the_schema() {
    let (_dir, plugin) = temp_plugin();
    let request = product_request(Some(
        r#"{"data":{"product":{"id":"10","name":"Backpack","status":"ACTIVE","internalSku":"X"}}}"#,
    ));

    let error = plugin
        .configure_interaction(Request::new(make_contents_config_with_type(
            &request,
            "application/graphql-response",
        )))
        .await
        .expect_err("configure fails")
        .message()
        .to_string();

    assert!(
        error.contains("internalSku") && error.contains("Product"),
        "error names the offending field: {error}"
    );
}

#[tokio::test]
async fn configure_rejects_a_response_call_without_response_body_json() {
    let (_dir, plugin) = temp_plugin();
    let request = product_request(None);

    let error = plugin
        .configure_interaction(Request::new(make_contents_config_with_type(
            &request,
            "application/graphql-response",
        )))
        .await
        .expect_err("configure fails")
        .message()
        .to_string();

    assert!(
        error.contains("response_body_json"),
        "error explains that response_body_json is required: {error}"
    );
}

#[tokio::test]
async fn compare_contents_reports_response_mismatches() {
    let (_dir, plugin) = temp_plugin();

    // Configure first so we have the plugin configuration to compare against.
    let request = product_request(Some(
        r#"{"data":{"product":{"id":"10","name":"Backpack","status":"ACTIVE"}}}"#,
    ));
    let configured = plugin
        .configure_interaction(Request::new(make_contents_config(&request)))
        .await
        .expect("configure succeeds")
        .into_inner();
    let plugin_configuration: PluginConfiguration = configured
        .plugin_configuration
        .expect("plugin configuration");

    let actual = r#"{"data":{"product":{"id":"10","name":"Backpack","status":"SOLD_OUT"}}}"#;
    let compared = plugin
        .compare_contents(Request::new(CompareContentsRequest {
            expected: None,
            actual: Some(Body {
                content_type: "application/graphql-response".to_string(),
                content: Some(actual.as_bytes().to_vec()),
                content_type_hint: 0,
            }),
            allow_unexpected_keys: false,
            rules: Default::default(),
            plugin_configuration: Some(plugin_configuration),
        }))
        .await
        .expect("compare succeeds")
        .into_inner();

    let mismatches = compared
        .results
        .get("$.data.product.status")
        .expect("a mismatch is reported for status");
    assert_eq!(mismatches.mismatches.len(), 1, "got {mismatches:#?}");
}
