use std::sync::Arc;

use anyhow::bail;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::encoder::Transport;
use crate::{SchemaRef, SchemaRegistry};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphqlPluginConfig {
    pub query_document: String,
    pub operation_name: Option<String>,
    pub variables_json: Option<String>,
    pub transport: Transport,
    pub schema_ref: Option<SchemaRef>,
    pub schema_inline_base64: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphqlPluginRequest {
    pub query_document: String,
    pub operation_name: Option<String>,
    pub variables_json: Option<String>,
    #[serde(default)]
    pub transport: Transport,
    pub schema_sdl: Option<String>,
}

impl Default for GraphqlPluginRequest {
    fn default() -> Self {
        Self {
            query_document: String::new(),
            operation_name: None,
            variables_json: None,
            transport: Transport::JsonBody,
            schema_sdl: None,
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

    pub fn build(&self, req: GraphqlPluginRequest) -> anyhow::Result<GraphqlPluginConfig> {
        if req.query_document.trim().is_empty() {
            bail!("query_document is required");
        }

        let GraphqlPluginRequest {
            query_document,
            operation_name,
            variables_json,
            transport,
            schema_sdl,
        } = req;

        let (schema_ref, schema_inline_base64) = match schema_sdl {
            Some(sdl) => {
                let reference = self.registry.store(&sdl)?;
                let inline_schema = self.registry.inline_schema(&reference.hash)?;
                let inline_base64 = BASE64_STANDARD.encode(inline_schema.as_bytes());

                (Some(reference), Some(inline_base64))
            }
            None => (None, None),
        };

        Ok(GraphqlPluginConfig {
            query_document,
            operation_name,
            variables_json,
            transport,
            schema_ref,
            schema_inline_base64,
        })
    }
}
