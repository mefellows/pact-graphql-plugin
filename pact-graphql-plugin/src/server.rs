use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use pact_plugin_driver::proto;
use pact_plugin_driver::proto::body::ContentTypeHint;
use pact_plugin_driver::proto::catalogue_entry::EntryType;
use pact_plugin_driver::proto::pact_plugin_server::{PactPlugin, PactPluginServer};
use pact_plugin_driver::proto::{
    Body, CatalogueEntry, CompareContentsRequest, CompareContentsResponse,
    ConfigureInteractionRequest, ConfigureInteractionResponse, ContentMismatch, ContentMismatches,
    GenerateContentRequest, GenerateContentResponse, InteractionData, InteractionResponse,
    MockServerRequest, MockServerResults, PluginConfiguration, ShutdownMockServerRequest,
    ShutdownMockServerResponse, StartMockServerRequest, StartMockServerResponse,
    VerificationPreparationRequest, VerificationPreparationResponse, VerificationResult,
    VerificationResultItem, VerifyInteractionRequest, VerifyInteractionResponse,
};
use pact_plugin_driver::utils::{proto_struct_to_json, to_proto_struct};
use prost_types::Struct;
use serde_json::Value;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::service::Interceptor;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use tracing::debug;
use uuid::Uuid;

use crate::encoder::{GraphqlRequest, RequestEncoder, Transport};
use crate::graphql_payload::{CanonicalGraphqlRequest, RequestMismatch};
use crate::interaction::{GraphqlInteractionBuilder, GraphqlPluginConfig, GraphqlPluginRequest};
use crate::schema::SchemaRegistry;

const DEFAULT_SCHEMA_DIR: &str = ".pact-graphql-plugin/schemas";
const PLUGIN_NAME: &str = "graphql";
const GRAPHQL_JSON_CONTENT_TYPE: &str = "application/graphql";
const GRAPHQL_QUERY_CONTENT_TYPE: &str = "application/graphql";
const GRAPHQL_VALIDATION_MISMATCH: &str = "GraphQL query validation failed";

pub struct GraphqlPlugin {
    builder: GraphqlInteractionBuilder,
}

impl GraphqlPlugin {
    pub fn new() -> Result<Self> {
        let registry = SchemaRegistry::new(default_schema_root()?)?;
        Ok(Self::with_registry(registry))
    }

    pub fn with_registry(registry: SchemaRegistry) -> Self {
        let registry = Arc::new(registry);
        let builder = GraphqlInteractionBuilder::new(registry);
        Self { builder }
    }

    fn configure(&self, req: &ConfigureInteractionRequest) -> Result<ConfigureInteractionResponse> {
        debug!(content_type = %req.content_type, "configure: building GraphQL interaction");
        let contents_struct = req
            .contents_config
            .as_ref()
            .ok_or_else(|| anyhow!("contents_config is required"))?;
        let json = proto_struct_to_json(contents_struct);
        let gql_request: GraphqlPluginRequest = serde_json::from_value(json)?;
        let config = self.builder.build(gql_request)?;
        let body = encode_body(&config)?;
        let plugin_cfg = PluginConfiguration {
            interaction_configuration: Some(config_to_struct(&config)?),
            pact_configuration: None,
        };

        Ok(ConfigureInteractionResponse {
            error: String::new(),
            interaction: vec![InteractionResponse {
                contents: Some(body),
                plugin_configuration: Some(plugin_cfg.clone()),
                ..InteractionResponse::default()
            }],
            plugin_configuration: Some(plugin_cfg),
        })
    }

    fn generate(&self, req: &GenerateContentRequest) -> Result<GenerateContentResponse> {
        debug!("generate: rebuilding canonical GraphQL request body");
        let plugin_config = req
            .plugin_configuration
            .as_ref()
            .and_then(|cfg| cfg.interaction_configuration.as_ref())
            .ok_or_else(|| anyhow!("plugin interaction configuration is required"))?;
        let json = proto_struct_to_json(plugin_config);
        let config: GraphqlPluginConfig = serde_json::from_value(json)?;
        let body = encode_body(&config)?;
        Ok(GenerateContentResponse {
            contents: Some(body),
        })
    }
}

#[tonic::async_trait]
impl PactPlugin for GraphqlPlugin {
    async fn init_plugin(
        &self,
        _request: Request<proto::InitPluginRequest>,
    ) -> Result<Response<proto::InitPluginResponse>, Status> {
        debug!("init_plugin: registering GraphQL matcher + generator");
        let catalogue = vec![
            CatalogueEntry {
                r#type: EntryType::ContentMatcher as i32,
                key: "graphql".to_string(),
                values: HashMap::from([(
                    "content-types".to_string(),
                    GRAPHQL_JSON_CONTENT_TYPE.to_string(),
                )]),
            },
            CatalogueEntry {
                r#type: EntryType::ContentGenerator as i32,
                key: "graphql".to_string(),
                values: HashMap::from([(
                    "content-types".to_string(),
                    GRAPHQL_JSON_CONTENT_TYPE.to_string(),
                )]),
            },
        ];

        Ok(Response::new(proto::InitPluginResponse { catalogue }))
    }

    async fn update_catalogue(
        &self,
        _request: Request<proto::Catalogue>,
    ) -> Result<Response<()>, Status> {
        debug!("update_catalogue: received core catalogue update");
        Ok(Response::new(()))
    }

    async fn compare_contents(
        &self,
        request: Request<CompareContentsRequest>,
    ) -> Result<Response<CompareContentsResponse>, Status> {
        debug!("compare_contents: invoked by Pact core");
        let req = request.into_inner();
        let CompareContentsRequest {
            actual,
            plugin_configuration,
            ..
        } = req;

        let plugin_struct = plugin_configuration
            .and_then(|cfg| cfg.interaction_configuration)
            .ok_or_else(|| {
                Status::invalid_argument("plugin interaction configuration is required")
            })?;
        let config_value = proto_struct_to_json(&plugin_struct);
        let config: GraphqlPluginConfig = serde_json::from_value(config_value)
            .map_err(|err| Status::invalid_argument(err.to_string()))?;

        let expected = CanonicalGraphqlRequest {
            payload: config.request.clone(),
            inline_schema: config.inline_schema.clone(),
            query_matching: config.query_matching,
        };

        let actual_body =
            actual.ok_or_else(|| Status::invalid_argument("actual request body is required"))?;
        let actual_bytes = actual_body
            .content
            .ok_or_else(|| Status::invalid_argument("actual request body is required"))?;

        let content_type = if actual_body.content_type.trim().is_empty() {
            default_content_type(&config.transport)
        } else {
            actual_body.content_type
        };

        let schema_indicator = schema_indicator(&config);
        let registry = self.builder.registry();
        debug!(
            transport = ?config.transport,
            content_type = %content_type,
            has_inline_schema = config.inline_schema.is_some(),
            "compare_contents: canonicalising actual request"
        );
        let actual = match CanonicalGraphqlRequest::from_http_request(
            &actual_bytes,
            &content_type,
            config.transport.clone(),
            schema_indicator.as_deref(),
            registry.as_ref(),
            config.query_matching,
        ) {
            Ok(actual) => actual,
            Err(err) => {
                if is_validation_error(&err) {
                    let mismatch = validation_mismatch_from_error(
                        &err,
                        &expected.payload.query_document,
                    );
                    let response = build_compare_contents_response(vec![mismatch]);
                    return Ok(Response::new(response));
                }
                return Err(to_status(err));
            }
        };

        let mismatches = expected.diff(&actual);
        debug!(
            mismatch_count = mismatches.len(),
            "compare_contents: diff complete"
        );
        let response = build_compare_contents_response(mismatches);
        Ok(Response::new(response))
    }

    async fn configure_interaction(
        &self,
        request: Request<ConfigureInteractionRequest>,
    ) -> Result<Response<ConfigureInteractionResponse>, Status> {
        write_log_marker("configure_interaction called");
        debug!("configure_interaction: request received from Pact core");
        let response = self.configure(request.get_ref()).map_err(to_status)?;
        Ok(Response::new(response))
    }

    async fn generate_content(
        &self,
        request: Request<GenerateContentRequest>,
    ) -> Result<Response<GenerateContentResponse>, Status> {
        write_log_marker("generate_content called");
        debug!("generate_content: request received from Pact core");
        let response = self.generate(request.get_ref()).map_err(to_status)?;
        Ok(Response::new(response))
    }

    async fn start_mock_server(
        &self,
        _request: Request<StartMockServerRequest>,
    ) -> Result<Response<StartMockServerResponse>, Status> {
        debug!("start_mock_server: not implemented for GraphQL plugin");
        Err(Status::unimplemented("mock servers are not supported"))
    }

    async fn shutdown_mock_server(
        &self,
        _request: Request<ShutdownMockServerRequest>,
    ) -> Result<Response<ShutdownMockServerResponse>, Status> {
        debug!("shutdown_mock_server: not implemented for GraphQL plugin");
        Err(Status::unimplemented("mock servers are not supported"))
    }

    async fn get_mock_server_results(
        &self,
        _request: Request<MockServerRequest>,
    ) -> Result<Response<MockServerResults>, Status> {
        debug!("get_mock_server_results: not implemented for GraphQL plugin");
        Err(Status::unimplemented("mock servers are not supported"))
    }

    async fn prepare_interaction_for_verification(
        &self,
        request: Request<VerificationPreparationRequest>,
    ) -> Result<Response<VerificationPreparationResponse>, Status> {
        write_log_marker("prepare_interaction_for_verification called");
        debug!(interaction_key = %request.get_ref().interaction_key, "prepare_interaction_for_verification: extracting canonical payload");
        let req = request.into_inner();
        let config = config_from_pact(&req.pact, &req.interaction_key).map_err(to_status)?;

        let body = encode_body(&config).map_err(to_status)?;
        let interaction_data = InteractionData {
            body: Some(body),
            metadata: HashMap::new(),
        };

        let response = VerificationPreparationResponse {
            response: Some(
                proto::verification_preparation_response::Response::InteractionData(
                    interaction_data,
                ),
            ),
        };

        Ok(Response::new(response))
    }

    async fn verify_interaction(
        &self,
        request: Request<VerifyInteractionRequest>,
    ) -> Result<Response<VerifyInteractionResponse>, Status> {
        write_log_marker("verify_interaction called");
        debug!(interaction_key = %request.get_ref().interaction_key, "verify_interaction: comparing provider response");
        let req = request.into_inner();
        let config = config_from_pact(&req.pact, &req.interaction_key).map_err(to_status)?;

        let expected = CanonicalGraphqlRequest {
            payload: config.request.clone(),
            inline_schema: config.inline_schema.clone(),
            query_matching: config.query_matching,
        };

        let interaction_data = req
            .interaction_data
            .ok_or_else(|| Status::invalid_argument("interaction_data is required"))?;
        let body = interaction_data
            .body
            .ok_or_else(|| Status::invalid_argument("interaction_data.body is required"))?;
        let bytes = body
            .content
            .ok_or_else(|| Status::invalid_argument("interaction_data.body content is required"))?;

        let content_type = if body.content_type.trim().is_empty() {
            default_content_type(&config.transport)
        } else {
            body.content_type
        };

        let schema_indicator = schema_indicator(&config);
        let registry = self.builder.registry();
        debug!(
            transport = ?config.transport,
            content_type = %content_type,
            "verify_interaction: canonicalising provider payload"
        );
        let actual = match CanonicalGraphqlRequest::from_http_request(
            &bytes,
            &content_type,
            config.transport.clone(),
            schema_indicator.as_deref(),
            registry.as_ref(),
            config.query_matching,
        ) {
            Ok(actual) => actual,
            Err(err) => {
                if is_validation_error(&err) {
                    let mismatch = validation_mismatch_from_error(
                        &err,
                        &expected.payload.query_document,
                    );
                    let response = build_verify_interaction_response(vec![mismatch]);
                    return Ok(Response::new(response));
                }
                return Err(to_status(err));
            }
        };

        let mismatches = expected.diff(&actual);
        debug!(
            mismatch_count = mismatches.len(),
            "verify_interaction: diff complete"
        );
        let response = build_verify_interaction_response(mismatches);
        Ok(Response::new(response))
    }
}

pub async fn run() -> Result<()> {
    let plugin = GraphqlPlugin::new()?;
    let host = std::env::var("PACT_PLUGIN_HOST").unwrap_or_else(|_| "[::1]".to_string());
    write_log_marker("run called");
    debug!(%host, "run: binding GraphQL plugin server");
    let listener = TcpListener::bind(format!("{}:0", host)).await?;
    let address: SocketAddr = listener.local_addr()?;
    debug!(%address, "run: GraphQL plugin listening");
    let server_key = Uuid::new_v4().to_string();
    println!(
        "{{\"port\":{}, \"serverKey\":\"{}\"}}",
        address.port(),
        server_key
    );

    let interceptor = AuthInterceptor {
        server_key: server_key.clone(),
    };
    let incoming = TcpListenerStream::new(listener);
    Server::builder()
        .add_service(PactPluginServer::with_interceptor(plugin, interceptor))
        .serve_with_incoming_shutdown(incoming, shutdown_signal())
        .await?;
    Ok(())
}

#[derive(Clone)]
struct AuthInterceptor {
    server_key: String,
}

impl Interceptor for AuthInterceptor {
    fn call(&mut self, request: Request<()>) -> Result<Request<()>, Status> {
        match request.metadata().get("authorization") {
            Some(token) if token == self.server_key.as_str() => Ok(request),
            Some(_) => Err(Status::unauthenticated("invalid server key")),
            None => Err(Status::unauthenticated("missing server key")),
        }
    }
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

fn encode_body(config: &GraphqlPluginConfig) -> Result<Body> {
    let request = match config.transport {
        Transport::JsonBody => GraphqlRequest::json(
            config.query_document.clone(),
            config.operation_name.clone(),
            config.variables_json.clone(),
        ),
        Transport::QueryString => GraphqlRequest::query_string(
            config.query_document.clone(),
            config.operation_name.clone(),
            config.variables_json.clone(),
        ),
    };
    let encoded = RequestEncoder::encode(&request)?;
    match config.transport {
        Transport::JsonBody => Ok(Body {
            content_type: GRAPHQL_JSON_CONTENT_TYPE.to_string(),
            content: encoded.body.map(|body| body.into_bytes()),
            content_type_hint: ContentTypeHint::Text as i32,
        }),
        Transport::QueryString => Ok(Body {
            content_type: GRAPHQL_QUERY_CONTENT_TYPE.to_string(),
            content: encoded.query_string.map(|body| body.into_bytes()),
            content_type_hint: ContentTypeHint::Text as i32,
        }),
    }
}

fn schema_indicator(config: &GraphqlPluginConfig) -> Option<String> {
    if let Some(inline) = &config.inline_schema {
        if let Some(base64) = &inline.base64_sdl {
            if !base64.trim().is_empty() {
                return Some(base64.clone());
            }
        }
    }

    if let Some(legacy) = &config.schema_inline_base64 {
        if !legacy.trim().is_empty() {
            return Some(legacy.clone());
        }
    }

    config
        .schema_ref
        .as_ref()
        .map(|reference| reference.hash.clone())
}

fn default_content_type(transport: &Transport) -> String {
    match transport {
        Transport::JsonBody => GRAPHQL_JSON_CONTENT_TYPE.to_string(),
        Transport::QueryString => GRAPHQL_QUERY_CONTENT_TYPE.to_string(),
    }
}

fn build_compare_contents_response(mismatches: Vec<RequestMismatch>) -> CompareContentsResponse {
    if mismatches.is_empty() {
        return CompareContentsResponse {
            error: String::new(),
            type_mismatch: None,
            results: HashMap::new(),
        };
    }

    let content_mismatches = mismatches
        .into_iter()
        .map(content_mismatch_from_request)
        .collect();

    let mut results = HashMap::new();
    results.insert(
        "$".to_string(),
        ContentMismatches {
            mismatches: content_mismatches,
        },
    );

    CompareContentsResponse {
        error: String::new(),
        type_mismatch: None,
        results,
    }
}

fn build_verify_interaction_response(
    mismatches: Vec<RequestMismatch>,
) -> VerifyInteractionResponse {
    if mismatches.is_empty() {
        return VerifyInteractionResponse {
            response: Some(proto::verify_interaction_response::Response::Result(
                VerificationResult {
                    success: true,
                    response_data: None,
                    mismatches: Vec::new(),
                    output: Vec::new(),
                },
            )),
        };
    }

    let items = mismatches
        .into_iter()
        .map(|mismatch| VerificationResultItem {
            result: Some(proto::verification_result_item::Result::Mismatch(
                content_mismatch_from_request(mismatch),
            )),
        })
        .collect();

    VerifyInteractionResponse {
        response: Some(proto::verify_interaction_response::Response::Result(
            VerificationResult {
                success: false,
                response_data: None,
                mismatches: items,
                output: Vec::new(),
            },
        )),
    }
}

fn config_from_pact(pact_json: &str, interaction_key: &str) -> Result<GraphqlPluginConfig> {
    let pact_value: Value = serde_json::from_str(pact_json).context("failed to parse pact JSON")?;
    let interactions = pact_value
        .get("interactions")
        .and_then(|value| value.as_array())
        .ok_or_else(|| anyhow!("pact JSON did not contain an interactions array"))?;

    let interaction = interactions
        .iter()
        .find(|interaction| {
            interaction
                .get("key")
                .or_else(|| interaction.get("interactionId"))
                .or_else(|| interaction.get("id"))
                .and_then(|value| value.as_str())
                .map(|key| key == interaction_key)
                .unwrap_or(false)
        })
        .ok_or_else(|| {
            anyhow!(
                "interaction with key '{}' not found in pact",
                interaction_key
            )
        })?;

    let plugin_config = interaction
        .get("pluginConfiguration")
        .and_then(|cfg| cfg.get(PLUGIN_NAME))
        .and_then(|cfg| cfg.get("interactionConfiguration"))
        .ok_or_else(|| {
            anyhow!(
                "plugin configuration for '{}' not found in pact interaction",
                PLUGIN_NAME
            )
        })?;

    let config: GraphqlPluginConfig = serde_json::from_value(plugin_config.clone())
        .context("failed to deserialize plugin configuration from pact")?;
    Ok(config)
}

fn content_mismatch_from_request(mismatch: RequestMismatch) -> ContentMismatch {
    ContentMismatch {
        expected: Some(mismatch.expected.into_bytes()),
        actual: Some(mismatch.actual.into_bytes()),
        mismatch: mismatch.description,
        path: mismatch.path,
        diff: mismatch.diff,
        mismatch_type: "body".to_string(),
    }
}

fn config_to_struct(config: &GraphqlPluginConfig) -> Result<Struct> {
    let value = serde_json::to_value(config)?;
    if let Value::Object(map) = value {
        let mut data = HashMap::new();
        for (k, v) in map {
            data.insert(k, v);
        }
        Ok(to_proto_struct(&data))
    } else {
        Err(anyhow!("configuration must serialize to an object"))
    }
}

fn default_schema_root() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("PACT_GRAPHQL_PLUGIN_SCHEMA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    Ok(std::env::current_dir()?.join(DEFAULT_SCHEMA_DIR))
}

fn write_log_marker(_message: &str) {}

fn to_status(err: anyhow::Error) -> Status {
    Status::invalid_argument(err.to_string())
}

fn is_validation_error(err: &anyhow::Error) -> bool {
    err.chain()
        .any(|cause| cause.to_string().contains(GRAPHQL_VALIDATION_MISMATCH))
}

fn validation_mismatch_from_error(err: &anyhow::Error, expected_query: &str) -> RequestMismatch {
    let diff = err
        .chain()
        .map(|cause| cause.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    RequestMismatch {
        path: "/payload/query_document".to_string(),
        expected: expected_query.to_string(),
        actual: String::new(),
        description: GRAPHQL_VALIDATION_MISMATCH.to_string(),
        diff,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::tempdir;
    use tokio::runtime::Runtime;
    use tonic::Request;

    use crate::encoder::{GraphqlRequest, RequestEncoder};
    use crate::graphql_payload::{GraphqlInlineSchema, GraphqlRequestPayload};
    use serde_json::json;

    #[test]
    fn config_roundtrip() {
        let inline_schema = Some(GraphqlInlineSchema {
            base64_sdl: Some("dGVzdA==".into()),
        });
        let config = GraphqlPluginConfig {
            query_document: "query".into(),
            operation_name: Some("op".into()),
            variables_json: Some("{}".into()),
            transport: Transport::JsonBody,
            schema_ref: None,
            schema_inline_base64: inline_schema
                .as_ref()
                .and_then(|schema| schema.base64_sdl.clone()),
            inline_schema: inline_schema.clone(),
            request: GraphqlRequestPayload {
                query_document: "query".into(),
                operation_name: Some("op".into()),
                variables_json: Some("{}".into()),
                transport: Transport::JsonBody,
            },
            query_matching: Default::default(),
        };
        let struct_value = config_to_struct(&config).unwrap();
        let value = proto_struct_to_json(&struct_value);
        let decoded: GraphqlPluginConfig = serde_json::from_value(value).unwrap();
        assert_eq!(decoded.query_document, "query");
        assert_eq!(decoded.transport, Transport::JsonBody);
        assert_eq!(decoded.request, config.request);
        assert_eq!(decoded.inline_schema, config.inline_schema);
        assert_eq!(decoded.schema_inline_base64, config.schema_inline_base64);
    }

    #[test]
    fn deserializes_request_from_flat_fields() {
        let value = json!({
            "query_document": "query",
            "operation_name": "MyOp",
            "variables_json": "{\"a\":1}",
            "transport": "json_body",
            "schema_ref": null,
            "schema_inline_base64": null
        });
        let config: GraphqlPluginConfig = serde_json::from_value(value).unwrap();
        assert_eq!(config.request.query_document, "query");
        assert_eq!(config.request.operation_name.as_deref(), Some("MyOp"));
        assert_eq!(config.request.variables_json.as_deref(), Some("{\"a\":1}"));
        assert_eq!(config.request.transport, Transport::JsonBody);
    }

    #[test]
    fn synthesizes_inline_schema_from_legacy_field() {
        let base64 = "dGVzdA==";
        let value = json!({
            "query_document": "query",
            "operation_name": null,
            "variables_json": null,
            "transport": "json_body",
            "schema_inline_base64": base64
        });
        let config: GraphqlPluginConfig = serde_json::from_value(value).unwrap();
        let synthesized = config
            .inline_schema
            .and_then(|schema| schema.base64_sdl)
            .expect("inline schema synthesized");
        assert_eq!(synthesized, base64);
    }

    #[test]
    fn serializes_inline_schema_back_to_legacy_field() {
        let config = GraphqlPluginConfig {
            query_document: "query".into(),
            operation_name: None,
            variables_json: None,
            transport: Transport::JsonBody,
            schema_ref: None,
            schema_inline_base64: None,
            inline_schema: Some(GraphqlInlineSchema {
                base64_sdl: Some("dGVzdA==".into()),
            }),
            request: GraphqlRequestPayload {
                query_document: "query".into(),
                operation_name: None,
                variables_json: None,
                transport: Transport::JsonBody,
            },
            query_matching: Default::default(),
        };
        let json_value = serde_json::to_value(&config).unwrap();
        assert_eq!(
            json_value
                .get("schema_inline_base64")
                .and_then(|v| v.as_str()),
            Some("dGVzdA==")
        );
    }

    #[test]
    fn deserializes_from_nested_payload_only() {
        let value = json!({
            "request": {
                "query_document": "query Products",
                "operation_name": "Op",
                "variables_json": "{\"a\":1}",
                "transport": "query_string"
            },
            "inline_schema": {
                "base64_sdl": "dGVzdA=="
            }
        });
        let config: GraphqlPluginConfig = serde_json::from_value(value).unwrap();
        assert_eq!(config.query_document, "query Products");
        assert_eq!(config.operation_name.as_deref(), Some("Op"));
        assert_eq!(config.variables_json.as_deref(), Some("{\"a\":1}"));
        assert_eq!(config.transport, Transport::QueryString);
        assert_eq!(
            config
                .inline_schema
                .as_ref()
                .and_then(|schema| schema.base64_sdl.as_deref()),
            Some("dGVzdA==")
        );
        assert_eq!(config.schema_inline_base64.as_deref(), Some("dGVzdA=="));
        assert_eq!(config.request.transport, Transport::QueryString);
    }

    #[test]
    fn compare_contents_matches() {
        let runtime = Runtime::new().unwrap();
        runtime.block_on(async {
            let tmp = tempdir().unwrap();
            let registry = SchemaRegistry::new(tmp.path()).unwrap();
            let plugin = GraphqlPlugin::with_registry(registry);
            let config = sample_plugin_config();
            let actual_request = GraphqlRequest::json(
                config.request.query_document.clone(),
                config.request.operation_name.clone(),
                config.request.variables_json.clone(),
            );
            let compare_request = build_compare_request(&config, &actual_request);

            let response = plugin
                .compare_contents(Request::new(compare_request))
                .await
                .expect("compare contents")
                .into_inner();

            assert!(response.results.is_empty(), "expected no mismatches");
        });
    }

    #[test]
    fn compare_contents_mismatch() {
        let runtime = Runtime::new().unwrap();
        runtime.block_on(async {
            let tmp = tempdir().unwrap();
            let registry = SchemaRegistry::new(tmp.path()).unwrap();
            let plugin = GraphqlPlugin::with_registry(registry);
            let config = sample_plugin_config();
            let actual_request = GraphqlRequest::json(
                "query Products { products { id } }",
                config.request.operation_name.clone(),
                config.request.variables_json.clone(),
            );
            let compare_request = build_compare_request(&config, &actual_request);

            let response = plugin
                .compare_contents(Request::new(compare_request))
                .await
                .expect("compare contents")
                .into_inner();

            assert_eq!(response.results.len(), 1);
            let mismatches = response.results.get("$").expect("mismatch entries");
            assert_eq!(mismatches.mismatches.len(), 1);
            let mismatch = &mismatches.mismatches[0];
            assert_eq!(mismatch.path, "/payload/query_document/products/name");
            assert!(
                mismatch.mismatch.contains("not selected by the actual query"),
                "{}",
                mismatch.mismatch
            );
        });
    }

    #[test]
    fn compare_contents_validation_mismatch() {
        let runtime = Runtime::new().unwrap();
        runtime.block_on(async {
            let tmp = tempdir().unwrap();
            let registry = SchemaRegistry::new(tmp.path()).unwrap();
            let plugin = GraphqlPlugin::with_registry(registry);
            let config = sample_plugin_config();
            let actual_request = GraphqlRequest::json(
                "query Products { products { id bogus } }",
                config.request.operation_name.clone(),
                config.request.variables_json.clone(),
            );
            let compare_request = build_compare_request(&config, &actual_request);

            let response = plugin
                .compare_contents(Request::new(compare_request))
                .await
                .expect("compare contents")
                .into_inner();

            assert_eq!(response.results.len(), 1);
            let mismatches = response.results.get("$").expect("mismatch entries");
            assert_eq!(mismatches.mismatches.len(), 1);
            let mismatch = &mismatches.mismatches[0];
            assert_eq!(mismatch.path, "/payload/query_document");
            assert_eq!(mismatch.mismatch, "GraphQL query validation failed");
            assert!(
                mismatch.diff.contains("GraphQL query validation failed"),
                "expected validation failure to be in diff"
            );
            assert!(
                mismatch.diff.contains("does not exist on type"),
                "expected field error to be in diff"
            );
        });
    }

    #[test]
    fn prepare_interaction_for_verification_returns_config() {
        let runtime = Runtime::new().unwrap();
        runtime.block_on(async {
            let tmp = tempdir().unwrap();
            let registry = SchemaRegistry::new(tmp.path()).unwrap();
            let plugin = GraphqlPlugin::with_registry(registry);
            let config = sample_plugin_config();
            let interaction_key = "prepare-key";
            let pact_json = pact_with_config(&config, interaction_key);

            let request = VerificationPreparationRequest {
                pact: pact_json,
                interaction_key: interaction_key.to_string(),
                config: None,
            };

            let response = plugin
                .prepare_interaction_for_verification(Request::new(request))
                .await
                .expect("prepare interaction")
                .into_inner();

            let body = match response.response.expect("response") {
                proto::verification_preparation_response::Response::InteractionData(data) => {
                    data.body.expect("body")
                }
                proto::verification_preparation_response::Response::Error(err) => {
                    panic!("unexpected error: {}", err)
                }
            };

            let expected_body = encode_body(&config).expect("encode body");
            assert_eq!(body.content_type, expected_body.content_type);
            assert_eq!(body.content, expected_body.content);
        });
    }

    #[test]
    fn verify_interaction_matches() {
        let runtime = Runtime::new().unwrap();
        runtime.block_on(async {
            let tmp = tempdir().unwrap();
            let registry = SchemaRegistry::new(tmp.path()).unwrap();
            let plugin = GraphqlPlugin::with_registry(registry);
            let config = sample_plugin_config();
            let interaction_key = "verify-match";
            let pact_json = pact_with_config(&config, interaction_key);

            let prepare_request = VerificationPreparationRequest {
                pact: pact_json.clone(),
                interaction_key: interaction_key.to_string(),
                config: None,
            };
            let prepared = plugin
                .prepare_interaction_for_verification(Request::new(prepare_request))
                .await
                .expect("prepare interaction")
                .into_inner();
            let interaction_data = match prepared.response.expect("response") {
                proto::verification_preparation_response::Response::InteractionData(data) => data,
                proto::verification_preparation_response::Response::Error(err) => {
                    panic!("unexpected error: {}", err)
                }
            };

            let verify_request = build_verify_request(interaction_key, pact_json, interaction_data);

            let response = plugin
                .verify_interaction(Request::new(verify_request))
                .await
                .expect("verify interaction")
                .into_inner();

            let result = match response.response.expect("response") {
                proto::verify_interaction_response::Response::Result(result) => result,
                proto::verify_interaction_response::Response::Error(err) => {
                    panic!("unexpected error: {}", err)
                }
            };

            assert!(result.success, "verification should succeed");
            assert!(result.mismatches.is_empty(), "no mismatches expected");
        });
    }

    #[test]
    fn verify_interaction_mismatch() {
        let runtime = Runtime::new().unwrap();
        runtime.block_on(async {
            let tmp = tempdir().unwrap();
            let registry = SchemaRegistry::new(tmp.path()).unwrap();
            let plugin = GraphqlPlugin::with_registry(registry);
            let config = sample_plugin_config();
            let interaction_key = "verify-mismatch";
            let pact_json = pact_with_config(&config, interaction_key);

            let prepare_request = VerificationPreparationRequest {
                pact: pact_json.clone(),
                interaction_key: interaction_key.to_string(),
                config: None,
            };
            let prepared = plugin
                .prepare_interaction_for_verification(Request::new(prepare_request))
                .await
                .expect("prepare interaction")
                .into_inner();
            let mut interaction_data = match prepared.response.expect("response") {
                proto::verification_preparation_response::Response::InteractionData(data) => data,
                proto::verification_preparation_response::Response::Error(err) => {
                    panic!("unexpected error: {}", err)
                }
            };

            let mutated_request = GraphqlRequest::json(
                "query Products { products { id } }",
                config.request.operation_name.clone(),
                config.request.variables_json.clone(),
            );
            interaction_data.body = Some(actual_body_from_request(&mutated_request));

            let verify_request = build_verify_request(interaction_key, pact_json, interaction_data);

            let response = plugin
                .verify_interaction(Request::new(verify_request))
                .await
                .expect("verify interaction")
                .into_inner();

            let result = match response.response.expect("response") {
                proto::verify_interaction_response::Response::Result(result) => result,
                proto::verify_interaction_response::Response::Error(err) => {
                    panic!("unexpected error: {}", err)
                }
            };

            assert!(!result.success, "verification should fail");
            assert_eq!(result.mismatches.len(), 1);
            let mismatch = match &result.mismatches[0].result {
                Some(proto::verification_result_item::Result::Mismatch(m)) => m,
                _ => panic!("expected mismatch result"),
            };
            assert_eq!(mismatch.path, "/payload/query_document/products/name");
            assert!(
                mismatch.mismatch.contains("not selected by the actual query"),
                "{}",
                mismatch.mismatch
            );
        });
    }

    #[test]
    fn verify_interaction_invalid_query() {
        let runtime = Runtime::new().unwrap();
        runtime.block_on(async {
            let tmp = tempdir().unwrap();
            let registry = SchemaRegistry::new(tmp.path()).unwrap();
            let plugin = GraphqlPlugin::with_registry(registry);
            let config = sample_plugin_config();
            let interaction_key = "verify-invalid";
            let pact_json = pact_with_config(&config, interaction_key);

            let prepare_request = VerificationPreparationRequest {
                pact: pact_json.clone(),
                interaction_key: interaction_key.to_string(),
                config: None,
            };
            let prepared = plugin
                .prepare_interaction_for_verification(Request::new(prepare_request))
                .await
                .expect("prepare interaction")
                .into_inner();
            let mut interaction_data = match prepared.response.expect("response") {
                proto::verification_preparation_response::Response::InteractionData(data) => data,
                proto::verification_preparation_response::Response::Error(err) => {
                    panic!("unexpected error: {}", err)
                }
            };

            let mutated_request = GraphqlRequest::json(
                "query Products { products { missingField } }",
                config.request.operation_name.clone(),
                config.request.variables_json.clone(),
            );
            interaction_data.body = Some(actual_body_from_request(&mutated_request));

            let verify_request = build_verify_request(interaction_key, pact_json, interaction_data);

            let response = plugin
                .verify_interaction(Request::new(verify_request))
                .await
                .expect("verify interaction")
                .into_inner();

            let result = match response.response.expect("response") {
                proto::verify_interaction_response::Response::Result(result) => result,
                proto::verify_interaction_response::Response::Error(err) => {
                    panic!("unexpected error: {}", err)
                }
            };

            assert!(!result.success, "verification should fail");
            assert_eq!(result.mismatches.len(), 1);
            let mismatch = match &result.mismatches[0].result {
                Some(proto::verification_result_item::Result::Mismatch(m)) => m,
                _ => panic!("expected mismatch result"),
            };
            assert_eq!(mismatch.path, "/payload/query_document");
            assert_eq!(mismatch.mismatch, "GraphQL query validation failed");
            assert!(
                mismatch.diff.contains("GraphQL query validation failed"),
                "expected validation failure to be in diff"
            );
        });
    }

    fn sample_plugin_config() -> GraphqlPluginConfig {
        let tmp = tempdir().unwrap();
        let registry = SchemaRegistry::new(tmp.path()).unwrap();
        let builder = GraphqlInteractionBuilder::new(Arc::new(registry));
        builder
            .build(GraphqlPluginRequest {
                query_document: "query Products { products { id name } }".into(),
                operation_name: Some("Products".into()),
                variables_json: None,
                transport: Transport::JsonBody,
                schema_sdl: Some(
                    r#"
                        type Query {
                            products: [Product!]!
                        }

                        type Product {
                            id: ID!
                            name: String!
                        }
                    "#
                    .into(),
                ),
                ..GraphqlPluginRequest::default()
            })
            .expect("canonical config")
    }

    fn build_compare_request(
        config: &GraphqlPluginConfig,
        actual_request: &GraphqlRequest,
    ) -> CompareContentsRequest {
        let rules = HashMap::new();
        CompareContentsRequest {
            expected: Some(encode_body(config).expect("encode expected body")),
            actual: Some(actual_body_from_request(actual_request)),
            allow_unexpected_keys: false,
            rules,
            plugin_configuration: Some(PluginConfiguration {
                interaction_configuration: Some(
                    config_to_struct(config).expect("encode plugin configuration"),
                ),
                pact_configuration: None,
            }),
        }
    }

    fn build_verify_request(
        interaction_key: &str,
        pact_json: String,
        interaction_data: InteractionData,
    ) -> VerifyInteractionRequest {
        VerifyInteractionRequest {
            interaction_data: Some(interaction_data),
            config: None,
            pact: pact_json,
            interaction_key: interaction_key.to_string(),
        }
    }

    fn pact_with_config(config: &GraphqlPluginConfig, interaction_key: &str) -> String {
        let config_value = serde_json::to_value(config).expect("config to JSON");
        serde_json::json!({
            "interactions": [
                {
                    "key": interaction_key,
                    "pluginConfiguration": {
                        PLUGIN_NAME: {
                            "interactionConfiguration": config_value
                        }
                    }
                }
            ]
        })
        .to_string()
    }

    fn actual_body_from_request(request: &GraphqlRequest) -> Body {
        let encoded = RequestEncoder::encode(request).expect("encode request");
        let payload = match request.transport {
            Transport::JsonBody => encoded.body.expect("json body payload").into_bytes(),
            Transport::QueryString => encoded
                .query_string
                .expect("query string payload")
                .into_bytes(),
        };

        Body {
            content_type: default_content_type(&request.transport),
            content: Some(payload),
            content_type_hint: ContentTypeHint::Text as i32,
        }
    }
}
