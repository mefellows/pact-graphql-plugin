use serde::{Deserialize, Serialize};

use crate::encoder::Transport;

/// Canonical request payload captured for each GraphQL interaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphqlRequestPayload {
    pub query_document: String,
    pub operation_name: Option<String>,
    pub variables_json: Option<String>,
    pub transport: Transport,
}

/// Inline SDL metadata associated with a GraphQL interaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GraphqlInlineSchema {
    #[serde(default)]
    pub base64_sdl: Option<String>,
}
