use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use pact_plugin_driver::proto;
use pact_plugin_driver::proto::body::ContentTypeHint;
use pact_plugin_driver::proto::catalogue_entry::EntryType;
use pact_plugin_driver::proto::pact_plugin_server::{PactPlugin, PactPluginServer};
use pact_plugin_driver::proto::{
    Body, CatalogueEntry, CompareContentsRequest, CompareContentsResponse,
    ConfigureInteractionRequest, ConfigureInteractionResponse, GenerateContentRequest,
    GenerateContentResponse, InteractionResponse, MockServerRequest, MockServerResults,
    PluginConfiguration, ShutdownMockServerRequest, ShutdownMockServerResponse,
    StartMockServerRequest, StartMockServerResponse, VerificationPreparationRequest,
    VerificationPreparationResponse, VerifyInteractionRequest, VerifyInteractionResponse,
};
use pact_plugin_driver::utils::{proto_struct_to_json, to_proto_struct};
use prost_types::Struct;
use serde_json::Value;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::service::Interceptor;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::encoder::{GraphqlRequest, RequestEncoder, Transport};
use crate::interaction::{GraphqlInteractionBuilder, GraphqlPluginConfig, GraphqlPluginRequest};
use crate::schema::SchemaRegistry;

const DEFAULT_SCHEMA_DIR: &str = ".pact-graphql-plugin/schemas";

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
                ..InteractionResponse::default()
            }],
            plugin_configuration: Some(plugin_cfg),
        })
    }

    fn generate(&self, req: &GenerateContentRequest) -> Result<GenerateContentResponse> {
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
        let catalogue = vec![
            CatalogueEntry {
                r#type: EntryType::ContentMatcher as i32,
                key: "graphql".to_string(),
                values: HashMap::from([(
                    "content-types".to_string(),
                    "application/json".to_string(),
                )]),
            },
            CatalogueEntry {
                r#type: EntryType::ContentGenerator as i32,
                key: "graphql".to_string(),
                values: HashMap::from([(
                    "content-types".to_string(),
                    "application/json".to_string(),
                )]),
            },
        ];

        Ok(Response::new(proto::InitPluginResponse { catalogue }))
    }

    async fn update_catalogue(
        &self,
        _request: Request<proto::Catalogue>,
    ) -> Result<Response<()>, Status> {
        Ok(Response::new(()))
    }

    async fn compare_contents(
        &self,
        _request: Request<CompareContentsRequest>,
    ) -> Result<Response<CompareContentsResponse>, Status> {
        Ok(Response::new(CompareContentsResponse {
            error: "compare_contents is not implemented".to_string(),
            ..CompareContentsResponse::default()
        }))
    }

    async fn configure_interaction(
        &self,
        request: Request<ConfigureInteractionRequest>,
    ) -> Result<Response<ConfigureInteractionResponse>, Status> {
        let response = self.configure(request.get_ref()).map_err(to_status)?;
        Ok(Response::new(response))
    }

    async fn generate_content(
        &self,
        request: Request<GenerateContentRequest>,
    ) -> Result<Response<GenerateContentResponse>, Status> {
        let response = self.generate(request.get_ref()).map_err(to_status)?;
        Ok(Response::new(response))
    }

    async fn start_mock_server(
        &self,
        _request: Request<StartMockServerRequest>,
    ) -> Result<Response<StartMockServerResponse>, Status> {
        Err(Status::unimplemented("mock servers are not supported"))
    }

    async fn shutdown_mock_server(
        &self,
        _request: Request<ShutdownMockServerRequest>,
    ) -> Result<Response<ShutdownMockServerResponse>, Status> {
        Err(Status::unimplemented("mock servers are not supported"))
    }

    async fn get_mock_server_results(
        &self,
        _request: Request<MockServerRequest>,
    ) -> Result<Response<MockServerResults>, Status> {
        Err(Status::unimplemented("mock servers are not supported"))
    }

    async fn prepare_interaction_for_verification(
        &self,
        _request: Request<VerificationPreparationRequest>,
    ) -> Result<Response<VerificationPreparationResponse>, Status> {
        Err(Status::unimplemented("verification is not implemented"))
    }

    async fn verify_interaction(
        &self,
        _request: Request<VerifyInteractionRequest>,
    ) -> Result<Response<VerifyInteractionResponse>, Status> {
        Err(Status::unimplemented("verification is not implemented"))
    }
}

pub async fn run() -> Result<()> {
    let plugin = GraphqlPlugin::new()?;
    let host = std::env::var("PACT_PLUGIN_HOST").unwrap_or_else(|_| "[::1]".to_string());
    let listener = TcpListener::bind(format!("{}:0", host)).await?;
    let address: SocketAddr = listener.local_addr()?;
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
            content_type: "application/json".to_string(),
            content: encoded.body.map(|body| body.into_bytes()),
            content_type_hint: ContentTypeHint::Text as i32,
        }),
        Transport::QueryString => Ok(Body {
            content_type: "application/x-www-form-urlencoded".to_string(),
            content: encoded.query_string.map(|body| body.into_bytes()),
            content_type_hint: ContentTypeHint::Text as i32,
        }),
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

fn to_status(err: anyhow::Error) -> Status {
    Status::invalid_argument(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_roundtrip() {
        let config = GraphqlPluginConfig {
            query_document: "query".into(),
            operation_name: Some("op".into()),
            variables_json: Some("{}".into()),
            transport: Transport::JsonBody,
            schema_ref: None,
            schema_inline_base64: None,
        };
        let struct_value = config_to_struct(&config).unwrap();
        let value = proto_struct_to_json(&struct_value);
        let decoded: GraphqlPluginConfig = serde_json::from_value(value).unwrap();
        assert_eq!(decoded.query_document, "query");
        assert_eq!(decoded.transport, Transport::JsonBody);
    }
}
