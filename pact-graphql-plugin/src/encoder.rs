use anyhow::{Context, Result};
use serde_json::Value;
use urlencoding::encode;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Transport {
    JsonBody,
    QueryString,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphqlRequest {
    pub query_document: String,
    pub operation_name: Option<String>,
    pub variables_json: Option<String>,
    pub transport: Transport,
}

impl GraphqlRequest {
    pub fn json<Q, O, V>(query: Q, operation_name: Option<O>, variables_json: Option<V>) -> Self
    where
        Q: Into<String>,
        O: Into<String>,
        V: Into<String>,
    {
        Self {
            query_document: query.into(),
            operation_name: operation_name.map(Into::into),
            variables_json: variables_json.map(Into::into),
            transport: Transport::JsonBody,
        }
    }

    pub fn query_string<Q, O, V>(
        query: Q,
        operation_name: Option<O>,
        variables_json: Option<V>,
    ) -> Self
    where
        Q: Into<String>,
        O: Into<String>,
        V: Into<String>,
    {
        Self {
            query_document: query.into(),
            operation_name: operation_name.map(Into::into),
            variables_json: variables_json.map(Into::into),
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
            Transport::QueryString => Ok(Self::encode_query_string(request)),
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
            let parsed: Value = serde_json::from_str(variables)
                .with_context(|| "variables_json must be valid JSON".to_string())?;
            payload.insert("variables".to_string(), parsed);
        }

        let body = Value::Object(payload);
        Ok(EncodedRequest {
            body: Some(serde_json::to_string(&body)?),
            query_string: None,
        })
    }

    fn encode_query_string(request: &GraphqlRequest) -> EncodedRequest {
        let mut params: Vec<(String, String)> = Vec::new();
        params.push(("query".to_string(), request.query_document.clone()));

        if let Some(operation_name) = &request.operation_name {
            params.push(("operationName".to_string(), operation_name.clone()));
        }

        if let Some(variables) = &request.variables_json {
            params.push(("variables".to_string(), variables.clone()));
        }

        let query_string = params
            .into_iter()
            .map(|(key, value)| format!("{}={}", key, percent_encode_form_value(&value)))
            .collect::<Vec<_>>()
            .join("&");

        EncodedRequest {
            body: None,
            query_string: Some(query_string),
        }
    }
}

fn percent_encode_form_value(value: &str) -> String {
    encode(value).replace("%20", "+")
}
