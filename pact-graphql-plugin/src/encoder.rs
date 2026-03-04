use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use urlencoding::encode;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    JsonBody,
    QueryString,
}

impl Default for Transport {
    fn default() -> Self {
        Transport::JsonBody
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphqlRequest {
    pub query_document: String,
    pub operation_name: Option<String>,
    pub variables_json: Option<String>,
    pub transport: Transport,
}

impl GraphqlRequest {
    pub fn json<Q>(query: Q, operation_name: Option<String>, variables_json: Option<String>) -> Self
    where
        Q: Into<String>,
    {
        Self {
            query_document: query.into(),
            operation_name,
            variables_json,
            transport: Transport::JsonBody,
        }
    }

    pub fn query_string<Q>(
        query: Q,
        operation_name: Option<String>,
        variables_json: Option<String>,
    ) -> Self
    where
        Q: Into<String>,
    {
        Self {
            query_document: query.into(),
            operation_name,
            variables_json,
            transport: Transport::QueryString,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EncodedRequest {
    pub body: Option<String>,
    pub query_string: Option<String>,
}

pub struct RequestEncoder;

impl RequestEncoder {
    pub fn encode(request: &GraphqlRequest) -> Result<EncodedRequest> {
        match request.transport {
            Transport::JsonBody => Self::encode_json(request),
            Transport::QueryString => Self::encode_query_string(request),
        }
    }

    fn encode_json(request: &GraphqlRequest) -> Result<EncodedRequest> {
        let mut payload = serde_json::Map::new();
        payload.insert(
            "query".to_string(),
            Value::String(request.query_document.clone()),
        );

        if let Some(operation_name) = &request.operation_name {
            payload.insert(
                "operationName".to_string(),
                Value::String(operation_name.clone()),
            );
        }

        if let Some(variables) = &request.variables_json {
            let parsed = parse_variables_json(variables)?;
            payload.insert("variables".to_string(), parsed);
        }

        let body = Value::Object(payload);
        Ok(EncodedRequest {
            body: Some(serde_json::to_string(&body)?),
            query_string: None,
        })
    }

    fn encode_query_string(request: &GraphqlRequest) -> Result<EncodedRequest> {
        let mut params: Vec<(String, String)> = Vec::new();
        params.push(("query".to_string(), request.query_document.clone()));

        if let Some(operation_name) = &request.operation_name {
            params.push(("operationName".to_string(), operation_name.clone()));
        }

        if let Some(variables) = &request.variables_json {
            let parsed = parse_variables_json(variables)?;
            let canonical = serde_json::to_string(&parsed)?;
            params.push(("variables".to_string(), canonical));
        }

        let query_string = params
            .into_iter()
            .map(|(key, value)| format!("{}={}", key, percent_encode_form_value(&value)))
            .collect::<Vec<_>>()
            .join("&");

        Ok(EncodedRequest {
            body: None,
            query_string: Some(query_string),
        })
    }
}

fn percent_encode_form_value(value: &str) -> String {
    encode(value).replace("%20", "+")
}

fn parse_variables_json(raw: &str) -> Result<Value> {
    serde_json::from_str(raw).with_context(|| "variables_json must be valid JSON".to_string())
}
