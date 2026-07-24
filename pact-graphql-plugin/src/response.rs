use std::collections::HashSet;

use anyhow::anyhow;
use graphql_parser::query::{Field as QueryField, OperationDefinition, SelectionSet};
use serde_json::Value;

use crate::query_ast::{self, collect_fields, selection_set_of};
use crate::schema_index::{OperationKind, SchemaIndex, TypeRef};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResponseMismatch {
    pub(crate) path: String,
    pub(crate) expected: String,
    pub(crate) actual: String,
    pub(crate) description: String,
}

impl ResponseMismatch {
    fn new(
        path: impl Into<String>,
        expected: impl Into<String>,
        actual: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            expected: expected.into(),
            actual: actual.into(),
            description: description.into(),
        }
    }
}

/// Short label for a JSON value, used in mismatch `actual` fields.
fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "list",
        Value::Object(_) => "object",
    }
}

fn operation_kind_of(operation: &OperationDefinition<'static, String>) -> OperationKind {
    match operation {
        OperationDefinition::Mutation(_) => OperationKind::Mutation,
        OperationDefinition::Subscription(_) => OperationKind::Subscription,
        OperationDefinition::Query(_) | OperationDefinition::SelectionSet(_) => {
            OperationKind::Query
        }
    }
}

pub(crate) fn validate_response(
    schema: &SchemaIndex,
    query_document: &str,
    operation_name: Option<&str>,
    response: &Value,
) -> anyhow::Result<Vec<ResponseMismatch>> {
    let operation = query_ast::parse_and_inline(query_document, operation_name)?;
    let root = schema
        .root_type(operation_kind_of(&operation))
        .ok_or_else(|| anyhow!("schema has no root type for this operation"))?
        .to_string();

    let mut mismatches = Vec::new();

    let Value::Object(envelope) = response else {
        mismatches.push(ResponseMismatch::new(
            "$",
            "object",
            json_kind(response),
            "a GraphQL response must be a JSON object",
        ));
        return Ok(mismatches);
    };

    const ALLOWED: [&str; 3] = ["data", "errors", "extensions"];
    for key in envelope.keys() {
        if !ALLOWED.contains(&key.as_str()) {
            mismatches.push(ResponseMismatch::new(
                format!("$.{key}"),
                "one of data, errors, extensions",
                key.clone(),
                format!("`{key}` is not a valid GraphQL response key"),
            ));
        }
    }

    if !envelope.contains_key("data") && !envelope.contains_key("errors") {
        mismatches.push(ResponseMismatch::new(
            "$",
            "data or errors",
            "neither",
            "a GraphQL response must contain `data`, `errors`, or both",
        ));
    }

    if let Some(errors) = envelope.get("errors") {
        validate_errors(errors, &mut mismatches);
    }

    match envelope.get("data") {
        None | Some(Value::Null) => {}
        Some(data) => validate_object(
            schema,
            selection_set_of(&operation),
            &root,
            data,
            "$.data",
            &mut mismatches,
        ),
    }

    Ok(mismatches)
}

fn validate_errors(errors: &Value, mismatches: &mut Vec<ResponseMismatch>) {
    let Value::Array(items) = errors else {
        mismatches.push(ResponseMismatch::new(
            "$.errors",
            "list",
            json_kind(errors),
            "`errors` must be a list",
        ));
        return;
    };

    for (index, item) in items.iter().enumerate() {
        let path = format!("$.errors[{index}]");
        let Value::Object(error) = item else {
            mismatches.push(ResponseMismatch::new(
                path,
                "object",
                json_kind(item),
                "each GraphQL error must be an object",
            ));
            continue;
        };
        match error.get("message") {
            Some(Value::String(_)) => {}
            other => mismatches.push(ResponseMismatch::new(
                format!("{path}.message"),
                "string",
                other.map(json_kind).unwrap_or("absent"),
                "each GraphQL error requires a string `message`",
            )),
        }
    }
}

/// Walks a JSON object against a selection set at `parent_type`.
fn validate_object(
    schema: &SchemaIndex,
    selection_set: &SelectionSet<'static, String>,
    parent_type: &str,
    value: &Value,
    path: &str,
    mismatches: &mut Vec<ResponseMismatch>,
) {
    let Value::Object(object) = value else {
        mismatches.push(ResponseMismatch::new(
            path,
            "object",
            json_kind(value),
            format!("expected an object for type `{parent_type}`"),
        ));
        return;
    };

    let selected: Vec<(String, &QueryField<'static, String>)> = collect_fields(selection_set);
    let selected_keys: HashSet<&str> = selected.iter().map(|(key, _)| key.as_str()).collect();

    for key in object.keys() {
        if key == "__typename" {
            continue;
        }
        if selected_keys.contains(key.as_str()) {
            continue;
        }
        let description = if schema.field_return_type(parent_type, key).is_some() {
            format!("`{key}` was not requested by the operation")
        } else {
            format!("`{key}` is not a field of type \"{parent_type}\"")
        };
        mismatches.push(ResponseMismatch::new(
            format!("{path}.{key}"),
            "<absent>",
            key.clone(),
            description,
        ));
    }

    for (key, field) in &selected {
        if field.name == "__typename" {
            continue;
        }
        let field_path = format!("{path}.{key}");
        let Some(return_type) = schema.field_return_type(parent_type, &field.name) else {
            // The query validator already rejects unknown selections; skip.
            continue;
        };
        match object.get(key) {
            None => mismatches.push(ResponseMismatch::new(
                field_path,
                key.clone(),
                "<absent>",
                format!("`{key}` was requested by the operation but is missing"),
            )),
            Some(field_value) => validate_value(
                schema,
                return_type,
                &field.selection_set,
                field_value,
                &field_path,
                mismatches,
            ),
        }
    }
}

fn validate_value(
    schema: &SchemaIndex,
    type_ref: &TypeRef,
    selection_set: &SelectionSet<'static, String>,
    value: &Value,
    path: &str,
    mismatches: &mut Vec<ResponseMismatch>,
) {
    if value.is_null() {
        if type_ref.is_non_null() {
            mismatches.push(ResponseMismatch::new(
                path,
                "a non-null value",
                "null",
                "field is non-null in the schema but the response is null",
            ));
        }
        return;
    }

    if let Some(item_type) = type_ref.as_list_item() {
        let Value::Array(items) = value else {
            mismatches.push(ResponseMismatch::new(
                path,
                "list",
                json_kind(value),
                "field is a list in the schema",
            ));
            return;
        };
        for (index, item) in items.iter().enumerate() {
            validate_value(
                schema,
                item_type,
                selection_set,
                item,
                &format!("{path}[{index}]"),
                mismatches,
            );
        }
        return;
    }

    let Some(named) = type_ref.unwrap_non_null().innermost_named() else {
        return;
    };

    if schema.is_composite_type(named) {
        validate_object(schema, selection_set, named, value, path, mismatches);
        return;
    }

    if let Some(members) = schema.enum_values(named) {
        match value {
            Value::String(actual) if members.contains(actual) => {}
            Value::String(actual) => {
                let mut allowed: Vec<&str> = members.iter().map(String::as_str).collect();
                allowed.sort_unstable();
                mismatches.push(ResponseMismatch::new(
                    path,
                    format!("one of {}", allowed.join(", ")),
                    actual.clone(),
                    format!("`{actual}` is not a member of enum `{named}`"),
                ));
            }
            other => mismatches.push(ResponseMismatch::new(
                path,
                format!("a `{named}` enum member (string)"),
                json_kind(other),
                format!("enum `{named}` must be serialised as a string"),
            )),
        }
        return;
    }

    validate_scalar(named, value, path, mismatches);
}

fn validate_scalar(
    named: &str,
    value: &Value,
    path: &str,
    mismatches: &mut Vec<ResponseMismatch>,
) {
    let ok = match named {
        "Int" => value.as_i64().is_some(),
        "Float" => value.is_number(),
        "Boolean" => value.is_boolean(),
        "String" | "ID" => value.is_string(),
        // Custom scalars have no defined JSON representation; accept anything
        // that is not a composite, which the schema already told us it is not.
        _ => true,
    };

    if !ok {
        mismatches.push(ResponseMismatch::new(
            path,
            format!("a `{named}` value"),
            json_kind(value),
            format!("value is not a valid `{named}`"),
        ));
    }
}

#[cfg(test)]
#[path = "response_tests.rs"]
mod tests;
