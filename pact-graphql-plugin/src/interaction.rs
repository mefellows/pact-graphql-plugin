use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::encoder::Transport;
use crate::query_ast::QueryMatching;
use crate::{SchemaRef, SchemaRegistry};

pub use crate::graphql_payload::{
    CanonicalGraphqlRequest, GraphqlInlineSchema, GraphqlRequestPayload,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphqlPluginConfig {
    pub query_document: String,
    pub operation_name: Option<String>,
    pub variables_json: Option<String>,
    pub transport: Transport,
    pub schema_ref: Option<SchemaRef>,
    pub schema_inline_base64: Option<String>,
    pub inline_schema: Option<GraphqlInlineSchema>,
    pub request: GraphqlRequestPayload,
    pub query_matching: QueryMatching,
    pub response_body_json: Option<String>,
}

/// Wire form of the interaction configuration, as stored in the pact file.
///
/// Absent fields are omitted rather than written as `null`: every one of these appears in every
/// interaction of every pact, and a missing `Option` deserialises to `None` regardless.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct GraphqlPluginConfigWire {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_document: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variables_json: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<Transport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_ref: Option<SchemaRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_inline_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inline_schema: Option<GraphqlInlineSchema>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<GraphqlRequestPayload>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_matching: Option<QueryMatching>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_body_json: Option<String>,
}

impl From<&GraphqlPluginConfig> for GraphqlPluginConfigWire {
    fn from(config: &GraphqlPluginConfig) -> Self {
        // `inline_schema.base64_sdl` is the canonical home for the SDL. The legacy
        // `schema_inline_base64` field held a byte-identical copy, doubling the largest thing in
        // every interaction's plugin configuration, so it is no longer written — only read, by
        // `Deserialize`, so that pacts recorded before this change still verify.
        let schema_inline_base64 = if config.inline_schema.is_some() {
            None
        } else {
            config.schema_inline_base64.clone()
        };
        Self {
            query_document: Some(config.query_document.clone()),
            operation_name: config.operation_name.clone(),
            variables_json: config.variables_json.clone(),
            transport: Some(config.transport.clone()),
            schema_ref: config.schema_ref.clone(),
            schema_inline_base64,
            inline_schema: config.inline_schema.clone(),
            request: Some(config.request.clone()),
            query_matching: Some(config.query_matching),
            response_body_json: config.response_body_json.clone(),
        }
    }
}

impl Serialize for GraphqlPluginConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let wire = GraphqlPluginConfigWire::from(self);
        wire.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GraphqlPluginConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = GraphqlPluginConfigWire::deserialize(deserializer)?;
        use serde::de::Error as SerdeError;

        let GraphqlPluginConfigWire {
            query_document,
            operation_name,
            variables_json,
            transport,
            schema_ref,
            mut schema_inline_base64,
            mut inline_schema,
            request,
            query_matching,
            response_body_json,
        } = wire;

        let request = request.unwrap_or_else(|| GraphqlRequestPayload {
            query_document: query_document.clone().unwrap_or_default(),
            operation_name: operation_name.clone(),
            variables_json: variables_json.clone(),
            transport: transport.clone().unwrap_or(Transport::JsonBody),
        });

        let query_document = query_document.unwrap_or_else(|| request.query_document.clone());
        if query_document.trim().is_empty() {
            return Err(SerdeError::custom("graphql config requires query_document"));
        }

        let operation_name = operation_name.or_else(|| request.operation_name.clone());
        let variables_json = variables_json.or_else(|| request.variables_json.clone());
        let transport = transport.unwrap_or_else(|| request.transport.clone());

        if schema_inline_base64.is_none() {
            schema_inline_base64 = inline_schema
                .as_ref()
                .and_then(|schema| schema.base64_sdl.clone());
        }
        if inline_schema.is_none() {
            if let Some(inline) = schema_inline_base64.clone() {
                inline_schema = Some(GraphqlInlineSchema {
                    base64_sdl: Some(inline),
                });
            }
        }

        Ok(Self {
            query_document,
            operation_name,
            variables_json,
            transport,
            schema_ref,
            schema_inline_base64,
            inline_schema,
            request,
            query_matching: query_matching.unwrap_or_default(),
            response_body_json,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphqlPluginRequest {
    pub query_document: String,
    pub operation_name: Option<String>,
    pub variables_json: Option<String>,
    #[serde(default)]
    pub transport: Transport,
    pub schema_sdl: Option<String>,
    #[serde(default)]
    pub query_matching: QueryMatching,
    #[serde(default)]
    pub response_body_json: Option<String>,
}

impl Default for GraphqlPluginRequest {
    fn default() -> Self {
        Self {
            query_document: String::new(),
            operation_name: None,
            variables_json: None,
            transport: Transport::JsonBody,
            schema_sdl: None,
            query_matching: QueryMatching::default(),
            response_body_json: None,
        }
    }
}

pub struct GraphqlInteractionBuilder {
    registry: Arc<SchemaRegistry>,
}

impl GraphqlInteractionBuilder {
    pub fn new(registry: Arc<SchemaRegistry>) -> Self {
        Self { registry }
    }

    pub fn registry(&self) -> Arc<SchemaRegistry> {
        Arc::clone(&self.registry)
    }

    pub fn build(&self, req: GraphqlPluginRequest) -> anyhow::Result<GraphqlPluginConfig> {
        let query_matching = req.query_matching;
        let response_body_json = req.response_body_json.clone();
        let canonical = CanonicalGraphqlRequest::from_interaction_config(req, &self.registry)?;

        let canonical_schema_sdl = canonical.inline_schema_sdl()?;
        let schema_ref = canonical_schema_sdl
            .as_deref()
            .map(|sdl| self.registry.store(sdl))
            .transpose()?;

        let inline_schema = canonical.inline_schema.clone();
        let schema_inline_base64 = inline_schema
            .as_ref()
            .and_then(|schema| schema.base64_sdl.clone());

        let request = canonical.payload.clone();

        Ok(GraphqlPluginConfig {
            query_document: request.query_document.clone(),
            operation_name: request.operation_name.clone(),
            variables_json: request.variables_json.clone(),
            transport: request.transport.clone(),
            schema_ref,
            schema_inline_base64,
            inline_schema,
            request,
            query_matching,
            response_body_json,
        })
    }
}
