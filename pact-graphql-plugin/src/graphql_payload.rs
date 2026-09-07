use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::str;

use anyhow::{anyhow, bail, Context};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use graphql_parser::query::{
    parse_query, Definition as QueryDefinition, Document as QueryDocument, Field as QueryField,
    FragmentDefinition, FragmentSpread, InlineFragment, OperationDefinition, Selection,
    SelectionSet, TypeCondition, Value as GqlValue,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use urlencoding::decode;

use crate::encoder::Transport;
use crate::interaction::GraphqlPluginRequest;
use crate::query_ast::QueryMatching;
use crate::schema_index::{FieldInfo, OperationKind, SchemaIndex, TypeInfo};
use crate::SchemaRegistry;

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

/// Canonical payload + inline schema resolved for configure-time/runtime use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalGraphqlRequest {
    pub payload: GraphqlRequestPayload,
    pub inline_schema: Option<GraphqlInlineSchema>,
    pub query_matching: QueryMatching,
}

/// Differentiates two canonical requests using JSON pointer paths.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestMismatch {
    pub path: String,
    pub expected: String,
    pub actual: String,
    pub description: String,
    pub diff: String,
}

impl RequestMismatch {
    pub(crate) fn new(path: &str, expected: String, actual: String, description: &str) -> Self {
        Self {
            path: path.to_string(),
            expected,
            actual,
            description: description.to_string(),
            diff: String::new(),
        }
    }
}

impl CanonicalGraphqlRequest {
    pub fn from_interaction_config(
        req: GraphqlPluginRequest,
        _registry: &SchemaRegistry,
    ) -> anyhow::Result<Self> {
        if req.query_document.trim().is_empty() {
            bail!("query_document is required");
        }

        let GraphqlPluginRequest {
            mut query_document,
            operation_name,
            variables_json,
            transport,
            schema_sdl,
            query_matching,
            response_body_json: _,
        } = req;

        query_document = canonicalize_query(&query_document, operation_name.as_deref())?;
        let variables_json = canonicalize_variables(variables_json)?;

        let canonical_schema = schema_sdl
            .map(|s| canonicalize_sdl(&s))
            .filter(|s| !s.is_empty());

        if let Some(ref canonical_sdl) = canonical_schema {
            let schema_index = SchemaIndex::from_sdl(canonical_sdl)
                .context("failed to parse GraphQL schema SDL")?;
            validate_against_schema(
                &schema_index,
                &query_document,
                operation_name.as_deref(),
                variables_json.as_deref(),
            )
            .context("GraphQL query validation failed")?;
        }

        let inline_schema = canonical_schema.map(|canonical_sdl| GraphqlInlineSchema {
            base64_sdl: Some(BASE64_STANDARD.encode(canonical_sdl.as_bytes())),
        });

        Ok(Self {
            payload: GraphqlRequestPayload {
                query_document,
                operation_name,
                variables_json,
                transport,
            },
            inline_schema,
            query_matching,
        })
    }

    pub fn from_http_request(
        body_bytes: &[u8],
        content_type: &str,
        expected_transport: Transport,
        schema_base64: Option<&str>,
        registry: &SchemaRegistry,
        query_matching: QueryMatching,
    ) -> anyhow::Result<Self> {
        let transport = detect_transport(content_type, expected_transport);
        let (raw_query, operation_name, raw_variables) = match transport {
            Transport::JsonBody => parse_json_graphql_body(body_bytes)?,
            Transport::QueryString => parse_query_string_graphql_body(body_bytes)?,
        };

        if raw_query.trim().is_empty() {
            bail!("query_document is required");
        }

        let query_document = canonicalize_query(&raw_query, operation_name.as_deref())?;
        let variables_json = canonicalize_variables(raw_variables)?;

        let canonical_schema = resolve_schema_sdl_indicator(schema_base64, registry)?;
        if let Some(ref canonical_sdl) = canonical_schema {
            let schema_index = SchemaIndex::from_sdl(canonical_sdl)
                .context("failed to parse GraphQL schema SDL")?;
            validate_against_schema(
                &schema_index,
                &query_document,
                operation_name.as_deref(),
                variables_json.as_deref(),
            )
            .context("GraphQL query validation failed")?;
        }

        let inline_schema = canonical_schema.map(|canonical_sdl| GraphqlInlineSchema {
            base64_sdl: Some(BASE64_STANDARD.encode(canonical_sdl.as_bytes())),
        });

        Ok(Self {
            payload: GraphqlRequestPayload {
                query_document,
                operation_name,
                variables_json,
                transport,
            },
            inline_schema,
            query_matching,
        })
    }

    pub fn inline_schema_sdl(&self) -> anyhow::Result<Option<String>> {
        let Some(schema) = &self.inline_schema else {
            return Ok(None);
        };
        let Some(base64) = &schema.base64_sdl else {
            return Ok(None);
        };
        let bytes = BASE64_STANDARD
            .decode(base64)
            .context("failed to decode inline schema base64")?;
        let sdl =
            String::from_utf8(bytes).context("inline schema base64 must decode to UTF-8 text")?;
        Ok(Some(sdl))
    }

    pub fn diff(&self, other: &CanonicalGraphqlRequest) -> Vec<RequestMismatch> {
        let mut mismatches = Vec::new();

        if self.query_matching == QueryMatching::Exact {
            for diff in crate::query_ast::diff_exact(
                &self.payload.query_document,
                &other.payload.query_document,
            ) {
                let path = if diff.path.is_empty() {
                    "/payload/query_document".to_string()
                } else {
                    format!("/payload/query_document/{}", diff.path.replace('.', "/"))
                };
                mismatches.push(RequestMismatch::new(
                    &path,
                    diff.expected,
                    diff.actual,
                    &diff.description,
                ));
            }
        } else {
            match (
                crate::query_ast::parse_and_inline(
                    &self.payload.query_document,
                    self.payload.operation_name.as_deref(),
                ),
                crate::query_ast::parse_and_inline(
                    &other.payload.query_document,
                    other.payload.operation_name.as_deref(),
                ),
            ) {
                (Ok(expected_op), Ok(actual_op)) => {
                    for diff in crate::query_ast::diff_operations_with(
                        &expected_op,
                        &actual_op,
                        self.query_matching,
                    ) {
                        let path = if diff.path.is_empty() {
                            "/payload/query_document".to_string()
                        } else {
                            format!("/payload/query_document/{}", diff.path.replace('.', "/"))
                        };
                        mismatches.push(RequestMismatch::new(
                            &path,
                            diff.expected,
                            diff.actual,
                            &diff.description,
                        ));
                    }
                }
                // If either document fails to parse here, fall back to the text comparison
                // so we still report *something* rather than silently passing.
                _ => {
                    if self.payload.query_document != other.payload.query_document {
                        mismatches.push(RequestMismatch::new(
                            "/payload/query_document",
                            self.payload.query_document.clone(),
                            other.payload.query_document.clone(),
                            "GraphQL query document differs",
                        ));
                    }
                }
            }
        }

        compare_optional_field(
            &mut mismatches,
            "/payload/operation_name",
            self.payload.operation_name.as_deref(),
            other.payload.operation_name.as_deref(),
            "GraphQL operation_name differs",
        );

        compare_optional_field(
            &mut mismatches,
            "/payload/variables_json",
            self.payload.variables_json.as_deref(),
            other.payload.variables_json.as_deref(),
            "GraphQL variables_json differs",
        );

        if self.payload.transport != other.payload.transport {
            mismatches.push(RequestMismatch::new(
                "/payload/transport",
                transport_label(&self.payload.transport).to_string(),
                transport_label(&other.payload.transport).to_string(),
                "transport differs",
            ));
        }

        compare_optional_field(
            &mut mismatches,
            "/inline_schema/base64_sdl",
            self.inline_schema
                .as_ref()
                .and_then(|schema| schema.base64_sdl.as_deref()),
            other
                .inline_schema
                .as_ref()
                .and_then(|schema| schema.base64_sdl.as_deref()),
            "inline schema differs",
        );

        mismatches
    }
}

fn compare_optional_field(
    mismatches: &mut Vec<RequestMismatch>,
    path: &str,
    expected: Option<&str>,
    actual: Option<&str>,
    description: &str,
) {
    if expected == actual {
        return;
    }
    mismatches.push(RequestMismatch::new(
        path,
        option_string(expected),
        option_string(actual),
        description,
    ));
}

fn option_string(value: Option<&str>) -> String {
    value
        .map(str::to_string)
        .unwrap_or_else(|| "<none>".to_string())
}

fn transport_label(transport: &Transport) -> &'static str {
    match transport {
        Transport::JsonBody => "json_body",
        Transport::QueryString => "query_string",
    }
}

fn parse_json_graphql_body(
    body_bytes: &[u8],
) -> anyhow::Result<(String, Option<String>, Option<String>)> {
    let value: Value =
        serde_json::from_slice(body_bytes).with_context(|| "failed to parse GraphQL JSON body")?;

    let query = match value.get("query") {
        Some(Value::String(query)) => query.clone(),
        Some(Value::Null) | None => {
            bail!("GraphQL JSON body missing required `query` field");
        }
        _ => bail!("GraphQL JSON field `query` must be a string"),
    };

    let operation_name = match value.get("operationName") {
        Some(Value::Null) | None => None,
        Some(Value::String(name)) if name.is_empty() => None,
        Some(Value::String(name)) => Some(name.clone()),
        _ => bail!("GraphQL JSON field `operationName` must be a string when present"),
    };

    let variables_json = match value.get("variables") {
        Some(Value::Null) | None => None,
        Some(Value::String(raw)) if raw.trim().is_empty() => None,
        Some(Value::String(raw)) => Some(raw.clone()),
        Some(other) => Some(serde_json::to_string(other)?),
    };

    Ok((query, operation_name, variables_json))
}

fn parse_query_string_graphql_body(
    body_bytes: &[u8],
) -> anyhow::Result<(String, Option<String>, Option<String>)> {
    let text = str::from_utf8(body_bytes)
        .with_context(|| "GraphQL query string payload must be valid UTF-8")?;

    let mut query = None;
    let mut operation_name = None;
    let mut variables = None;

    for pair in text.split('&') {
        if pair.is_empty() {
            continue;
        }
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or("");
        let value = parts.next().unwrap_or("");
        let decoded = decode_form_value(value)?;
        match key {
            "query" => query = Some(decoded),
            "operationName" => {
                operation_name = if decoded.is_empty() {
                    None
                } else {
                    Some(decoded)
                };
            }
            "variables" => {
                variables = if decoded.trim().is_empty() {
                    None
                } else {
                    Some(decoded)
                };
            }
            _ => {}
        }
    }

    let query =
        query.ok_or_else(|| anyhow!("GraphQL query string body missing `query` parameter"))?;
    Ok((query, operation_name, variables))
}

fn decode_form_value(value: &str) -> anyhow::Result<String> {
    let replaced = value.replace('+', " ");
    Ok(decode(&replaced)
        .map_err(|err| anyhow!("failed to decode form value: {err}"))?
        .into_owned())
}

fn detect_transport(content_type: &str, fallback: Transport) -> Transport {
    let lowered = content_type.to_ascii_lowercase();
    if lowered.contains("application/graphql") || lowered.contains("application/json") {
        Transport::JsonBody
    } else if lowered.contains("application/x-www-form-urlencoded") {
        Transport::QueryString
    } else {
        fallback
    }
}

fn resolve_schema_sdl_indicator(
    schema_base64: Option<&str>,
    registry: &SchemaRegistry,
) -> anyhow::Result<Option<String>> {
    match schema_base64.map(str::trim).filter(|s| !s.is_empty()) {
        Some(indicator) if is_schema_hash(indicator) => {
            let sdl = registry.inline_schema(indicator)?;
            let canonical = canonicalize_sdl(&sdl);
            if canonical.is_empty() {
                Ok(None)
            } else {
                Ok(Some(canonical))
            }
        }
        Some(encoded) => {
            let bytes = BASE64_STANDARD
                .decode(encoded)
                .with_context(|| "failed to decode inline schema base64")?;
            let sdl = String::from_utf8(bytes)
                .with_context(|| "inline schema base64 must decode to UTF-8 text")?;
            let canonical = canonicalize_sdl(&sdl);
            if canonical.is_empty() {
                Ok(None)
            } else {
                Ok(Some(canonical))
            }
        }
        None => Ok(None),
    }
}

fn is_schema_hash(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

pub(crate) type FragmentMap<'a> = HashMap<&'a str, &'a FragmentDefinition<'a, String>>;

/// Runs every schema-aware check we can make at configure time: the selection set against the
/// schema, and the supplied variables against the operation's declared variable definitions.
fn validate_against_schema(
    schema_index: &SchemaIndex,
    query_document: &str,
    operation_name: Option<&str>,
    variables_json: Option<&str>,
) -> anyhow::Result<()> {
    validate_query_document(schema_index, query_document, operation_name)?;

    // `validate_query_document` has already proved the document parses and is valid against the
    // schema. `parse_and_inline` can still decline to pick an operation for a multi-operation
    // document with no `operation_name`; there is no single set of variable definitions to check
    // in that case, so skip rather than fail.
    let Ok(operation) = crate::query_ast::parse_and_inline(query_document, operation_name) else {
        return Ok(());
    };

    let variables: Option<Value> = variables_json
        .map(serde_json::from_str)
        .transpose()
        .context("failed to parse GraphQL variables_json as JSON")?;

    crate::variables::validate_variables(schema_index, &operation, variables.as_ref())
}

pub(crate) fn validate_query_document(
    schema_index: &SchemaIndex,
    query_document: &str,
    operation_name: Option<&str>,
) -> anyhow::Result<()> {
    let document = parse_query::<String>(query_document)
        .with_context(|| "failed to parse GraphQL query document")?;
    let fragments = collect_fragments(&document);
    let operations = select_operations(&document, operation_name)?;

    for operation in operations {
        validate_operation(schema_index, operation, &fragments)?;
    }

    Ok(())
}

fn collect_fragments<'a>(document: &'a QueryDocument<'a, String>) -> FragmentMap<'a> {
    let mut fragments = HashMap::new();
    for definition in &document.definitions {
        if let QueryDefinition::Fragment(fragment) = definition {
            fragments.insert(fragment.name.as_str(), fragment);
        }
    }
    fragments
}

fn select_operations<'a>(
    document: &'a QueryDocument<'a, String>,
    operation_name: Option<&str>,
) -> anyhow::Result<Vec<&'a OperationDefinition<'a, String>>> {
    let operations: Vec<_> = document
        .definitions
        .iter()
        .filter_map(|definition| match definition {
            QueryDefinition::Operation(op) => Some(op),
            _ => None,
        })
        .collect();

    if operations.is_empty() {
        bail!("graphql query_document must define at least one operation");
    }

    if let Some(name) = operation_name {
        let operation = operations
            .into_iter()
            .find(|op| operation_definition_name(op) == Some(name))
            .ok_or_else(|| anyhow!("operation `{}` not found in query_document", name))?;
        Ok(vec![operation])
    } else {
        if operations.len() > 1 {
            bail!("operation_name is required when query_document defines multiple operations");
        }
        Ok(operations)
    }
}

fn operation_definition_name<'a>(
    operation: &'a OperationDefinition<'a, String>,
) -> Option<&'a str> {
    match operation {
        OperationDefinition::Query(query) => query.name.as_deref(),
        OperationDefinition::Mutation(mutation) => mutation.name.as_deref(),
        OperationDefinition::Subscription(subscription) => subscription.name.as_deref(),
        OperationDefinition::SelectionSet(_) => None,
    }
}

fn selection_set_for_operation<'a>(
    operation: &'a OperationDefinition<'a, String>,
) -> &'a SelectionSet<'a, String> {
    match operation {
        OperationDefinition::Query(query) => &query.selection_set,
        OperationDefinition::Mutation(mutation) => &mutation.selection_set,
        OperationDefinition::Subscription(subscription) => &subscription.selection_set,
        OperationDefinition::SelectionSet(selection_set) => selection_set,
    }
}

fn operation_kind(operation: &OperationDefinition<'_, String>) -> OperationKind {
    match operation {
        OperationDefinition::Mutation(_) => OperationKind::Mutation,
        OperationDefinition::Subscription(_) => OperationKind::Subscription,
        OperationDefinition::SelectionSet(_) | OperationDefinition::Query(_) => {
            OperationKind::Query
        }
    }
}

fn validate_operation<'a>(
    schema_index: &SchemaIndex,
    operation: &'a OperationDefinition<'a, String>,
    fragments: &FragmentMap<'a>,
) -> anyhow::Result<()> {
    let kind = operation_kind(operation);
    let root = schema_index
        .root_type(kind)
        .ok_or_else(|| anyhow!("no root type found for {:?}", kind))?;
    schema_index.ensure_type_exists(root)?;
    let mut fragment_stack = Vec::new();
    validate_selection_set(
        schema_index,
        selection_set_for_operation(operation),
        root,
        fragments,
        &mut fragment_stack,
    )
}

fn validate_selection_set<'a>(
    schema_index: &SchemaIndex,
    selection_set: &'a SelectionSet<'a, String>,
    parent_type: &str,
    fragments: &FragmentMap<'a>,
    fragment_stack: &mut Vec<&'a str>,
) -> anyhow::Result<()> {
    if selection_set.items.is_empty() {
        return Ok(());
    }

    let parent_info = schema_index
        .type_info(parent_type)
        .ok_or_else(|| anyhow!("type `{}` not found in schema", parent_type))?;

    if !schema_index.is_composite_type(parent_type) {
        bail!(
            "type `{}` is not a composite type and cannot have a selection set",
            parent_type
        );
    }

    for selection in &selection_set.items {
        match selection {
            Selection::Field(field) => {
                validate_field_selection(
                    schema_index,
                    parent_type,
                    parent_info,
                    field,
                    fragments,
                    fragment_stack,
                )?;
            }
            Selection::FragmentSpread(spread) => {
                validate_fragment_spread(
                    schema_index,
                    parent_type,
                    fragments,
                    spread,
                    fragment_stack,
                )?;
            }
            Selection::InlineFragment(fragment) => {
                validate_inline_fragment(
                    schema_index,
                    parent_type,
                    fragments,
                    fragment,
                    fragment_stack,
                )?;
            }
        }
    }

    Ok(())
}

fn validate_field_selection<'a>(
    schema_index: &SchemaIndex,
    parent_type: &str,
    parent_info: &TypeInfo,
    field: &'a QueryField<'a, String>,
    fragments: &FragmentMap<'a>,
    fragment_stack: &mut Vec<&'a str>,
) -> anyhow::Result<()> {
    if field.name == "__typename" {
        if !schema_index.is_composite_type(parent_type) {
            bail!(
                "`__typename` cannot be selected on non-composite type `{}`",
                parent_type
            );
        }
        if !field.selection_set.items.is_empty() {
            bail!("`__typename` does not support a selection set");
        }
        return Ok(());
    }

    let has_subselection = !field.selection_set.items.is_empty();

    if schema_index.is_query_root(parent_type) {
        match field.name.as_str() {
            "__schema" => {
                if !has_subselection {
                    bail!("`__schema` requires a selection set");
                }
                return validate_selection_set(
                    schema_index,
                    &field.selection_set,
                    "__Schema",
                    fragments,
                    fragment_stack,
                );
            }
            "__type" => {
                if !has_subselection {
                    bail!("`__type` requires a selection set");
                }
                validate_introspection_type_arguments(field)?;
                return validate_selection_set(
                    schema_index,
                    &field.selection_set,
                    "__Type",
                    fragments,
                    fragment_stack,
                );
            }
            _ => {}
        }
    }

    if schema_index.is_union_type(parent_type) {
        bail!(
            "fields cannot be selected directly on union type `{}`; use fragments instead",
            parent_type
        );
    }

    let field_info = schema_index
        .field(parent_info, &field.name)
        .ok_or_else(|| unknown_field_error(parent_type, &field.name))?;
    validate_field_arguments(schema_index, field, field_info, parent_type)?;
    if let Some(next_type) = field_info.composite_type(schema_index) {
        if !has_subselection {
            bail!(
                "field `{}` on type `{}` requires a selection set",
                field.name,
                parent_type
            );
        }
        validate_selection_set(
            schema_index,
            &field.selection_set,
            next_type,
            fragments,
            fragment_stack,
        )
    } else {
        if has_subselection {
            bail!(
                "field `{}` on type `{}` does not support a selection set",
                field.name,
                parent_type
            );
        }
        Ok(())
    }
}

fn validate_fragment_spread<'a>(
    schema_index: &SchemaIndex,
    parent_type: &str,
    fragments: &FragmentMap<'a>,
    spread: &'a FragmentSpread<'a, String>,
    fragment_stack: &mut Vec<&'a str>,
) -> anyhow::Result<()> {
    let fragment = fragments
        .get(spread.fragment_name.as_str())
        .ok_or_else(|| anyhow!("fragment `{}` is not defined", spread.fragment_name))?;

    let type_name = match &fragment.type_condition {
        TypeCondition::On(name) => name.as_str(),
    };
    schema_index.ensure_type_exists(type_name)?;
    schema_index.ensure_fragment_applicable(parent_type, type_name)?;

    detect_fragment_cycle(fragment_stack, fragment.name.as_str())?;
    fragment_stack.push(fragment.name.as_str());
    validate_selection_set(
        schema_index,
        &fragment.selection_set,
        type_name,
        fragments,
        fragment_stack,
    )?;
    fragment_stack.pop();
    Ok(())
}

fn validate_inline_fragment<'a>(
    schema_index: &SchemaIndex,
    parent_type: &str,
    fragments: &FragmentMap<'a>,
    fragment: &'a InlineFragment<'a, String>,
    fragment_stack: &mut Vec<&'a str>,
) -> anyhow::Result<()> {
    let target_type = match &fragment.type_condition {
        Some(TypeCondition::On(name)) => {
            let type_name = name.as_str();
            schema_index.ensure_type_exists(type_name)?;
            schema_index.ensure_fragment_applicable(parent_type, type_name)?;
            type_name
        }
        None => {
            schema_index.ensure_type_exists(parent_type)?;
            parent_type
        }
    };
    validate_selection_set(
        schema_index,
        &fragment.selection_set,
        target_type,
        fragments,
        fragment_stack,
    )
}

fn unknown_field_error(parent: &str, field: &str) -> anyhow::Error {
    anyhow!("field `{}` does not exist on type `{}`", field, parent)
}

/// Checks a literal argument value against the argument's declared type. Mirrors the input
/// coercion rules applied to variables in `crate::variables`, over the query AST's value type
/// rather than JSON.
///
/// A `$variable` reference is accepted here: the value it carries is checked against its own
/// declaration separately.
fn validate_argument_value(
    schema_index: &SchemaIndex,
    type_ref: &crate::schema_index::TypeRef,
    value: &GqlValue<'_, String>,
    arg_name: &str,
    field_name: &str,
    parent_type: &str,
) -> anyhow::Result<()> {
    let mismatch = |expected: &str| {
        anyhow!(
            "argument `{}` on field `{}` of type `{}` expects {}, but got {}",
            arg_name,
            field_name,
            parent_type,
            expected,
            describe_ast_value(value)
        )
    };

    if matches!(value, GqlValue::Variable(_)) {
        return Ok(());
    }

    if matches!(value, GqlValue::Null) {
        if type_ref.is_non_null() {
            bail!(
                "argument `{}` on field `{}` of type `{}` does not accept null",
                arg_name,
                field_name,
                parent_type
            );
        }
        return Ok(());
    }

    if let Some(item_type) = type_ref.as_list_item() {
        let GqlValue::List(items) = value else {
            return Err(mismatch("a list"));
        };
        for item in items {
            validate_argument_value(
                schema_index,
                item_type,
                item,
                arg_name,
                field_name,
                parent_type,
            )?;
        }
        return Ok(());
    }

    let Some(named) = type_ref.unwrap_non_null().innermost_named() else {
        return Ok(());
    };
    let Some(info) = schema_index.type_info(named) else {
        return Ok(());
    };

    match info {
        TypeInfo::Enum(_) => {
            let GqlValue::Enum(member) = value else {
                return Err(mismatch(&format!("a value of enum `{named}`")));
            };
            if let Some(members) = schema_index.enum_values(named) {
                if !members.contains(member.as_str()) {
                    let mut allowed: Vec<&str> = members.iter().map(String::as_str).collect();
                    allowed.sort_unstable();
                    bail!(
                        "argument `{}` on field `{}` of type `{}` is `{}`, which is not a value of enum `{}` (expected one of: {})",
                        arg_name,
                        field_name,
                        parent_type,
                        member,
                        named,
                        allowed.join(", ")
                    );
                }
            }
            Ok(())
        }
        TypeInfo::InputObject => match value {
            GqlValue::Object(_) => Ok(()),
            _ => Err(mismatch(&format!("input object `{named}`"))),
        },
        TypeInfo::Scalar => {
            let ok = match named {
                "Int" => matches!(value, GqlValue::Int(_)),
                "Float" => matches!(value, GqlValue::Int(_) | GqlValue::Float(_)),
                "String" => matches!(value, GqlValue::String(_)),
                "ID" => matches!(value, GqlValue::String(_) | GqlValue::Int(_)),
                "Boolean" => matches!(value, GqlValue::Boolean(_)),
                // A custom scalar's literal representation is not described by the schema.
                _ => true,
            };
            if ok {
                Ok(())
            } else {
                Err(mismatch(&format!("a `{named}`")))
            }
        }
        _ => Ok(()),
    }
}

fn describe_ast_value(value: &GqlValue<'_, String>) -> &'static str {
    match value {
        GqlValue::Variable(_) => "a variable",
        GqlValue::Int(_) => "an integer",
        GqlValue::Float(_) => "a float",
        GqlValue::String(_) => "a string",
        GqlValue::Boolean(_) => "a boolean",
        GqlValue::Null => "null",
        GqlValue::Enum(_) => "an enum value",
        GqlValue::List(_) => "a list",
        GqlValue::Object(_) => "an object",
    }
}

fn validate_field_arguments(
    schema_index: &SchemaIndex,
    field: &QueryField<'_, String>,
    field_info: &FieldInfo,
    parent_type: &str,
) -> anyhow::Result<()> {
    let mut provided = HashSet::new();
    for (name, value) in &field.arguments {
        let arg_name = name.as_str();
        let Some(argument) = field_info.argument(arg_name) else {
            bail!(
                "argument `{}` is not defined on field `{}` of type `{}`",
                arg_name,
                field.name,
                parent_type
            );
        };
        validate_argument_value(
            schema_index,
            argument.type_ref(),
            value,
            arg_name,
            &field.name,
            parent_type,
        )?;
        provided.insert(arg_name.to_string());
    }

    for (name, argument) in &field_info.arguments {
        if argument.is_required() && !provided.contains(name) {
            bail!(
                "field `{}` on type `{}` is missing required argument `{}`",
                field.name,
                parent_type,
                name
            );
        }
    }

    Ok(())
}

fn validate_introspection_type_arguments(field: &QueryField<'_, String>) -> anyhow::Result<()> {
    let mut has_name = false;
    for (name, _) in &field.arguments {
        if name == "name" {
            has_name = true;
        } else {
            bail!(
                "argument `{}` is not defined on field `__type` of type `Query`",
                name
            );
        }
    }
    if !has_name {
        bail!("field `__type` on type `Query` is missing required argument `name`");
    }
    Ok(())
}

fn detect_fragment_cycle(stack: &[&str], next: &str) -> anyhow::Result<()> {
    if let Some(position) = stack.iter().position(|name| *name == next) {
        return Err(crate::query_ast::fragment_cycle_error(
            &stack[position..],
            next,
        ));
    }
    Ok(())
}

/// Canonical text form of a query document.
///
/// A document that is not syntactically valid GraphQL keeps its dedented raw
/// text: the validation path reports syntax errors with far better messages
/// than we could here. Every other failure — an unknown operation name, an
/// ambiguous multi-operation document, an unresolvable fragment — is a real
/// error the caller must see, so it propagates.
pub(crate) fn canonicalize_query(
    input: &str,
    operation_name: Option<&str>,
) -> anyhow::Result<String> {
    if graphql_parser::query::parse_query::<String>(input).is_err() {
        return Ok(dedent_and_trim(input));
    }
    crate::query_ast::canonical_document(input, operation_name)
}

fn strip_indent(line: &str, indent: usize) -> String {
    if indent == 0 {
        return line.to_string();
    }

    let mut skipped = 0usize;
    let mut result = String::with_capacity(line.len());
    for ch in line.chars() {
        if skipped < indent {
            if ch == ' ' || ch == '\t' {
                skipped += 1;
                continue;
            } else {
                skipped = indent;
            }
        }
        result.push(ch);
    }
    result
}

pub(crate) fn canonicalize_variables(
    variables_json: Option<String>,
) -> anyhow::Result<Option<String>> {
    match variables_json {
        Some(raw) => {
            let mut value: Value = serde_json::from_str(&raw)
                .with_context(|| "failed to parse GraphQL variables_json as JSON")?;
            sort_object_keys(&mut value);
            let canonical = serde_json::to_string(&value)?;
            Ok(Some(canonical))
        }
        None => Ok(None),
    }
}

/// Recursively sorts object keys so that variables differing only in key order canonicalise
/// identically. `variables_json` is compared as an exact string, and two objects with the same
/// entries in different orders are the same GraphQL variables — without this, a client that
/// builds variables from an unordered map (Go's `map[string]any`, say) would flake.
///
/// This cannot rely on `serde_json`'s default sorted `BTreeMap`, because this crate enables the
/// `preserve_order` feature, which swaps in an insertion-ordered `IndexMap`.
///
/// Array order is left alone: it is semantically meaningful in GraphQL variables.
fn sort_object_keys(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (_, entry) in map.iter_mut() {
                sort_object_keys(entry);
            }
            let mut entries: Vec<(String, Value)> = std::mem::take(map).into_iter().collect();
            entries.sort_by(|(a, _), (b, _)| a.cmp(b));
            for (key, entry) in entries {
                map.insert(key, entry);
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                sort_object_keys(item);
            }
        }
        _ => {}
    }
}

pub(crate) fn canonicalize_sdl(input: &str) -> String {
    let dedented = dedent_and_trim(input);
    if dedented.is_empty() {
        String::new()
    } else {
        let mut result = dedented;
        result.push('\n');
        result
    }
}

fn dedent_and_trim(input: &str) -> String {
    let normalized = normalize_newlines(input);
    let lines: Vec<&str> = normalized.split('\n').collect();

    let mut min_indent = usize::MAX;
    for line in &lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let indent = line.chars().take_while(|c| matches!(c, ' ' | '\t')).count();
        min_indent = min_indent.min(indent);
    }

    if min_indent == usize::MAX {
        min_indent = 0;
    }

    let mut dedented_lines = Vec::with_capacity(lines.len());
    for line in &lines {
        if line.trim().is_empty() {
            dedented_lines.push(String::new());
        } else {
            dedented_lines.push(strip_indent(line, min_indent));
        }
    }

    dedented_lines.join("\n").trim().to_string()
}

fn normalize_newlines(input: &str) -> Cow<'_, str> {
    if input.contains('\r') {
        Cow::Owned(input.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        Cow::Borrowed(input)
    }
}

#[cfg(test)]
#[path = "graphql_payload_tests.rs"]
mod tests;
