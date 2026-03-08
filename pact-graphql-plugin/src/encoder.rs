use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::form_urlencoded;
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

    pub fn decode(
        body: &[u8],
        _content_type: &str,
        expected_transport: &Transport,
    ) -> Result<GraphqlRequest> {
        match expected_transport {
            Transport::JsonBody => decode_json_body(body),
            Transport::QueryString => decode_query_string(body),
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

fn decode_json_body(body: &[u8]) -> Result<GraphqlRequest> {
    #[derive(Deserialize)]
    struct RawJsonBody {
        query: String,
        #[serde(rename = "operationName")]
        operation_name: Option<String>,
        variables: Option<Value>,
    }

    let parsed: RawJsonBody = serde_json::from_slice(body)
        .with_context(|| "failed to decode GraphQL JSON body".to_string())?;

    let variables_json = match parsed.variables {
        Some(Value::Null) | None => None,
        Some(value) => Some(serde_json::to_string(&value)?),
    };

    Ok(GraphqlRequest {
        query_document: parsed.query,
        operation_name: parsed.operation_name.filter(|s| !s.trim().is_empty()),
        variables_json,
        transport: Transport::JsonBody,
    })
}

fn decode_query_string(body: &[u8]) -> Result<GraphqlRequest> {
    let mut query = None;
    let mut operation_name = None;
    let mut variables = None;

    for (key, value) in form_urlencoded::parse(body) {
        match key.as_ref() {
            "query" => query = Some(value.into_owned()),
            "operationName" => operation_name = Some(value.into_owned()),
            "variables" => variables = Some(value.into_owned()),
            _ => {}
        }
    }

    let query_document = query
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow!("GraphQL query string body missing `query` parameter"))?;

    Ok(GraphqlRequest {
        query_document,
        operation_name: operation_name.filter(|s| !s.is_empty()),
        variables_json: variables.filter(|s| !s.is_empty()),
        transport: Transport::QueryString,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_json_body_roundtrip() {
        let request = GraphqlRequest::json(
            "query Products { products { id } }",
            Some("GetProducts".into()),
            Some("{\"size\":10}".into()),
        );
        let encoded = RequestEncoder::encode(&request).expect("encode json");
        let body = encoded.body.expect("encoded json body");
        let decoded =
            RequestEncoder::decode(body.as_bytes(), "application/json", &Transport::JsonBody)
                .expect("decode json");
        assert_eq!(decoded, request);
    }

    #[test]
    fn decode_query_string_roundtrip() {
        let request = GraphqlRequest::query_string(
            "query Products { products { id } }",
            None,
            Some("{\"size\":10}".into()),
        );
        let encoded = RequestEncoder::encode(&request).expect("encode query string");
        let query_string = encoded.query_string.expect("encoded query string");
        let decoded = RequestEncoder::decode(
            query_string.as_bytes(),
            "application/x-www-form-urlencoded",
            &Transport::QueryString,
        )
        .expect("decode query string");
        assert_eq!(decoded, request);
    }
}
