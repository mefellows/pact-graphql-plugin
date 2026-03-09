use std::borrow::Cow;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::str;

use anyhow::{anyhow, bail, Context};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use graphql_parser::query::{
    parse_query, Definition as QueryDefinition, Document as QueryDocument, Field as QueryField,
    FragmentDefinition, FragmentSpread, InlineFragment, OperationDefinition, Selection,
    SelectionSet, TypeCondition,
};
use graphql_parser::schema::{
    parse_schema, Definition as SchemaDefinition, Document as SchemaDocument, Field as SchemaField,
    InputValue as SchemaInputValue, Type as SchemaType, TypeDefinition, TypeExtension,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use urlencoding::decode;

use crate::encoder::Transport;
use crate::interaction::GraphqlPluginRequest;
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
    fn new(path: &str, expected: String, actual: String, description: &str) -> Self {
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
        } = req;

        query_document = canonicalize_query(&query_document);
        let variables_json = canonicalize_variables(variables_json)?;

        let canonical_schema = schema_sdl
            .map(|s| canonicalize_sdl(&s))
            .filter(|s| !s.is_empty());

        if let Some(ref canonical_sdl) = canonical_schema {
            let schema_index = SchemaIndex::from_sdl(canonical_sdl)
                .context("failed to parse GraphQL schema SDL")?;
            validate_query_document(&schema_index, &query_document, operation_name.as_deref())
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
        })
    }

    pub fn from_http_request(
        body_bytes: &[u8],
        content_type: &str,
        expected_transport: Transport,
        schema_base64: Option<&str>,
        registry: &SchemaRegistry,
    ) -> anyhow::Result<Self> {
        let transport = detect_transport(content_type, expected_transport);
        let (raw_query, operation_name, raw_variables) = match transport {
            Transport::JsonBody => parse_json_graphql_body(body_bytes)?,
            Transport::QueryString => parse_query_string_graphql_body(body_bytes)?,
        };

        if raw_query.trim().is_empty() {
            bail!("query_document is required");
        }

        let query_document = canonicalize_query(&raw_query);
        let variables_json = canonicalize_variables(raw_variables)?;

        let canonical_schema = resolve_schema_sdl_indicator(schema_base64, registry)?;
        if let Some(ref canonical_sdl) = canonical_schema {
            let schema_index = SchemaIndex::from_sdl(canonical_sdl)
                .context("failed to parse GraphQL schema SDL")?;
            validate_query_document(&schema_index, &query_document, operation_name.as_deref())
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

        if self.payload.query_document != other.payload.query_document {
            mismatches.push(RequestMismatch::new(
                "/payload/query_document",
                self.payload.query_document.clone(),
                other.payload.query_document.clone(),
                "GraphQL query document differs",
            ));
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

const INTROSPECTION_SDL: &str = r#"
type __Schema {
  description: String
  types: [__Type!]!
  queryType: __Type!
  mutationType: __Type
  subscriptionType: __Type
  directives: [__Directive!]!
}

type __Type {
  kind: __TypeKind!
  name: String
  description: String
  fields(includeDeprecated: Boolean = false): [__Field!]
  interfaces: [__Type!]
  possibleTypes: [__Type!]
  enumValues(includeDeprecated: Boolean = false): [__EnumValue!]
  inputFields(includeDeprecated: Boolean = false): [__InputValue!]
  ofType: __Type
  specifiedByURL: String
}

type __Field {
  name: String!
  description: String
  args(includeDeprecated: Boolean = false): [__InputValue!]!
  type: __Type!
  isDeprecated: Boolean!
  deprecationReason: String
}

type __InputValue {
  name: String!
  description: String
  type: __Type!
  defaultValue: String
  isDeprecated: Boolean
  deprecationReason: String
}

type __EnumValue {
  name: String!
  description: String
  isDeprecated: Boolean!
  deprecationReason: String
}

type __Directive {
  name: String!
  description: String
  locations: [__DirectiveLocation!]!
  args(includeDeprecated: Boolean = false): [__InputValue!]!
  isRepeatable: Boolean!
}

enum __DirectiveLocation {
  QUERY
  MUTATION
  SUBSCRIPTION
  FIELD
  FRAGMENT_DEFINITION
  FRAGMENT_SPREAD
  INLINE_FRAGMENT
  VARIABLE_DEFINITION
  SCHEMA
  SCALAR
  OBJECT
  FIELD_DEFINITION
  ARGUMENT_DEFINITION
  INTERFACE
  UNION
  ENUM
  ENUM_VALUE
  INPUT_OBJECT
  INPUT_FIELD_DEFINITION
}

enum __TypeKind {
  SCALAR
  OBJECT
  INTERFACE
  UNION
  ENUM
  INPUT_OBJECT
  LIST
  NON_NULL
}
"#;

pub(crate) type FragmentMap<'a> = HashMap<&'a str, &'a FragmentDefinition<'a, String>>;

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

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum OperationKind {
    Query,
    Mutation,
    Subscription,
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
    validate_field_arguments(field, field_info, parent_type)?;
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

fn validate_field_arguments(
    field: &QueryField<'_, String>,
    field_info: &FieldInfo,
    parent_type: &str,
) -> anyhow::Result<()> {
    let mut provided = HashSet::new();
    for (name, _) in &field.arguments {
        let arg_name = name.as_str();
        if field_info.argument(arg_name).is_none() {
            bail!(
                "argument `{}` is not defined on field `{}` of type `{}`",
                arg_name,
                field.name,
                parent_type
            );
        }
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
        let mut cycle: Vec<&str> = stack[position..].to_vec();
        cycle.push(next);
        bail!("fragment `{}` forms a cycle: {}", next, cycle.join(" -> "));
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub(crate) struct SchemaIndex {
    query_root: String,
    mutation_root: Option<String>,
    subscription_root: Option<String>,
    types: HashMap<String, TypeInfo>,
}

impl SchemaIndex {
    pub(crate) fn from_sdl(sdl: &str) -> anyhow::Result<Self> {
        let mut index = SchemaIndex {
            query_root: "Query".to_string(),
            mutation_root: None,
            subscription_root: None,
            types: HashMap::new(),
        };

        let introspection_doc = parse_schema::<String>(INTROSPECTION_SDL)
            .expect("introspection SDL must parse")
            .into_static();
        index.register_document(introspection_doc)?;

        if !sdl.trim().is_empty() {
            let user_document = parse_schema::<String>(sdl)
                .with_context(|| "failed to parse GraphQL schema SDL")?
                .into_static();
            index.register_document(user_document)?;
        }

        Ok(index)
    }

    fn register_document(
        &mut self,
        document: SchemaDocument<'static, String>,
    ) -> anyhow::Result<()> {
        for definition in document.definitions {
            match definition {
                SchemaDefinition::SchemaDefinition(schema_def) => {
                    self.apply_schema_definition(schema_def);
                }
                SchemaDefinition::TypeDefinition(type_def) => {
                    self.register_type(type_def)?;
                }
                SchemaDefinition::TypeExtension(extension) => {
                    self.apply_type_extension(extension)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn apply_schema_definition(
        &mut self,
        schema_def: graphql_parser::schema::SchemaDefinition<'static, String>,
    ) {
        if let Some(name) = schema_def.query {
            self.query_root = name;
        }
        if let Some(name) = schema_def.mutation {
            self.mutation_root = Some(name);
        }
        if let Some(name) = schema_def.subscription {
            self.subscription_root = Some(name);
        }
    }

    fn register_type(&mut self, type_def: TypeDefinition<'static, String>) -> anyhow::Result<()> {
        match type_def {
            TypeDefinition::Object(object) => {
                let fields = FieldCollection::from_fields(object.fields);
                let implements = object
                    .implements_interfaces
                    .into_iter()
                    .collect::<HashSet<_>>();
                match self.types.entry(object.name.clone()) {
                    Entry::Vacant(entry) => {
                        entry.insert(TypeInfo::Object(ObjectTypeInfo { fields, implements }));
                    }
                    Entry::Occupied(mut entry) => match entry.get_mut() {
                        TypeInfo::Object(existing) => {
                            existing.fields.extend_collection(fields);
                            existing.implements.extend(implements);
                        }
                        other => {
                            bail!(
                                "type `{}` redeclared as object but previously defined as {}",
                                object.name,
                                other.kind()
                            );
                        }
                    },
                }
            }
            TypeDefinition::Interface(interface) => {
                let fields = FieldCollection::from_fields(interface.fields);
                let implements = interface
                    .implements_interfaces
                    .into_iter()
                    .collect::<HashSet<_>>();
                match self.types.entry(interface.name.clone()) {
                    Entry::Vacant(entry) => {
                        entry.insert(TypeInfo::Interface(InterfaceTypeInfo {
                            fields,
                            implements,
                        }));
                    }
                    Entry::Occupied(mut entry) => match entry.get_mut() {
                        TypeInfo::Interface(existing) => {
                            existing.fields.extend_collection(fields);
                            existing.implements.extend(implements);
                        }
                        other => {
                            bail!(
                                "type `{}` redeclared as interface but previously defined as {}",
                                interface.name,
                                other.kind()
                            );
                        }
                    },
                }
            }
            TypeDefinition::Union(union) => {
                let members = union.types.into_iter().collect::<HashSet<_>>();
                match self.types.entry(union.name.clone()) {
                    Entry::Vacant(entry) => {
                        entry.insert(TypeInfo::Union(UnionTypeInfo { members }));
                    }
                    Entry::Occupied(mut entry) => match entry.get_mut() {
                        TypeInfo::Union(existing) => {
                            existing.members.extend(members);
                        }
                        other => {
                            bail!(
                                "type `{}` redeclared as union but previously defined as {}",
                                union.name,
                                other.kind()
                            );
                        }
                    },
                }
            }
            TypeDefinition::Scalar(scalar) => match self.types.entry(scalar.name.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(TypeInfo::Scalar);
                }
                Entry::Occupied(entry) => {
                    if !matches!(entry.get(), TypeInfo::Scalar) {
                        bail!(
                            "type `{}` redeclared as scalar but previously defined as {}",
                            scalar.name,
                            entry.get().kind()
                        );
                    }
                }
            },
            TypeDefinition::Enum(enum_type) => match self.types.entry(enum_type.name.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(TypeInfo::Enum);
                }
                Entry::Occupied(entry) => {
                    if !matches!(entry.get(), TypeInfo::Enum) {
                        bail!(
                            "type `{}` redeclared as enum but previously defined as {}",
                            enum_type.name,
                            entry.get().kind()
                        );
                    }
                }
            },
            TypeDefinition::InputObject(input) => match self.types.entry(input.name.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(TypeInfo::InputObject);
                }
                Entry::Occupied(entry) => {
                    if !matches!(entry.get(), TypeInfo::InputObject) {
                        bail!(
                            "type `{}` redeclared as input object but previously defined as {}",
                            input.name,
                            entry.get().kind()
                        );
                    }
                }
            },
        }
        Ok(())
    }

    fn apply_type_extension(
        &mut self,
        extension: TypeExtension<'static, String>,
    ) -> anyhow::Result<()> {
        match extension {
            TypeExtension::Object(object_ext) => {
                let entry = self
                    .types
                    .entry(object_ext.name.clone())
                    .or_insert_with(|| TypeInfo::Object(ObjectTypeInfo::default()));
                if let TypeInfo::Object(obj) = entry {
                    obj.fields.extend_fields(object_ext.fields);
                    obj.implements
                        .extend(object_ext.implements_interfaces.into_iter());
                } else {
                    bail!(
                        "type `{}` cannot be extended as object because it was previously defined as {}",
                        object_ext.name,
                        entry.kind()
                    );
                }
            }
            TypeExtension::Interface(interface_ext) => {
                let entry = self
                    .types
                    .entry(interface_ext.name.clone())
                    .or_insert_with(|| TypeInfo::Interface(InterfaceTypeInfo::default()));
                if let TypeInfo::Interface(interface) = entry {
                    interface.fields.extend_fields(interface_ext.fields);
                    interface
                        .implements
                        .extend(interface_ext.implements_interfaces.into_iter());
                } else {
                    bail!(
                        "type `{}` cannot be extended as interface because it was previously defined as {}",
                        interface_ext.name,
                        entry.kind()
                    );
                }
            }
            TypeExtension::Union(union_ext) => {
                let entry = self
                    .types
                    .entry(union_ext.name.clone())
                    .or_insert_with(|| TypeInfo::Union(UnionTypeInfo::default()));
                if let TypeInfo::Union(union) = entry {
                    union.members.extend(union_ext.types.into_iter());
                } else {
                    bail!(
                        "type `{}` cannot be extended as union because it was previously defined as {}",
                        union_ext.name,
                        entry.kind()
                    );
                }
            }
            TypeExtension::Scalar(scalar_ext) => {
                let entry = self
                    .types
                    .entry(scalar_ext.name.clone())
                    .or_insert_with(|| TypeInfo::Scalar);
                if !matches!(entry, TypeInfo::Scalar) {
                    bail!(
                        "type `{}` cannot be extended as scalar because it was previously defined as {}",
                        scalar_ext.name,
                        entry.kind()
                    );
                }
            }
            TypeExtension::Enum(enum_ext) => {
                let entry = self
                    .types
                    .entry(enum_ext.name.clone())
                    .or_insert_with(|| TypeInfo::Enum);
                if !matches!(entry, TypeInfo::Enum) {
                    bail!(
                        "type `{}` cannot be extended as enum because it was previously defined as {}",
                        enum_ext.name,
                        entry.kind()
                    );
                }
            }
            TypeExtension::InputObject(input_ext) => {
                let entry = self
                    .types
                    .entry(input_ext.name.clone())
                    .or_insert_with(|| TypeInfo::InputObject);
                if !matches!(entry, TypeInfo::InputObject) {
                    bail!(
                        "type `{}` cannot be extended as input object because it was previously defined as {}",
                        input_ext.name,
                        entry.kind()
                    );
                }
            }
        }
        Ok(())
    }

    pub(crate) fn root_type(&self, kind: OperationKind) -> Option<&str> {
        match kind {
            OperationKind::Query => Some(self.query_root.as_str()),
            OperationKind::Mutation => self.mutation_root.as_deref(),
            OperationKind::Subscription => self.subscription_root.as_deref(),
        }
    }

    fn type_info(&self, type_name: &str) -> Option<&TypeInfo> {
        self.types.get(type_name)
    }

    fn is_composite_type(&self, type_name: &str) -> bool {
        matches!(
            self.types.get(type_name),
            Some(TypeInfo::Object(_) | TypeInfo::Interface(_) | TypeInfo::Union(_))
        )
    }

    fn is_union_type(&self, type_name: &str) -> bool {
        matches!(self.types.get(type_name), Some(TypeInfo::Union(_)))
    }

    fn is_query_root(&self, type_name: &str) -> bool {
        self.query_root == type_name
    }

    fn ensure_type_exists(&self, type_name: &str) -> anyhow::Result<()> {
        if self.types.contains_key(type_name) {
            Ok(())
        } else {
            bail!("type `{}` not found in schema", type_name)
        }
    }

    fn field<'a>(&'a self, parent: &'a TypeInfo, field_name: &str) -> Option<&'a FieldInfo> {
        match parent {
            TypeInfo::Object(object) => object.fields.get(field_name),
            TypeInfo::Interface(interface) => interface.fields.get(field_name),
            _ => None,
        }
    }

    fn runtime_types(&self, type_name: &str) -> anyhow::Result<HashSet<String>> {
        match self.types.get(type_name) {
            Some(TypeInfo::Object(_)) => Ok(HashSet::from([type_name.to_string()])),
            Some(TypeInfo::Interface(_)) => self.interface_runtime_types(type_name),
            Some(TypeInfo::Union(union)) => Ok(union.members.clone()),
            Some(TypeInfo::Scalar) | Some(TypeInfo::Enum) | Some(TypeInfo::InputObject) => {
                Ok(HashSet::new())
            }
            None => bail!("type `{}` not found in schema", type_name),
        }
    }

    fn interface_runtime_types(&self, interface: &str) -> anyhow::Result<HashSet<String>> {
        if !matches!(self.types.get(interface), Some(TypeInfo::Interface(_))) {
            bail!("type `{}` is not an interface", interface);
        }
        let mut runtime = HashSet::new();
        for (name, info) in &self.types {
            if let TypeInfo::Object(object) = info {
                if self.object_implements_interface(object, interface) {
                    runtime.insert(name.clone());
                }
            }
        }
        Ok(runtime)
    }

    fn object_implements_interface(&self, object: &ObjectTypeInfo, target: &str) -> bool {
        if object.implements.contains(target) {
            return true;
        }
        let mut stack: Vec<String> = object.implements.iter().cloned().collect();
        let mut visited = HashSet::new();
        while let Some(interface_name) = stack.pop() {
            if interface_name == target {
                return true;
            }
            if !visited.insert(interface_name.clone()) {
                continue;
            }
            if let Some(TypeInfo::Interface(interface)) = self.types.get(&interface_name) {
                if interface.implements.contains(target) {
                    return true;
                }
                for parent in &interface.implements {
                    if parent == target {
                        return true;
                    }
                    stack.push(parent.clone());
                }
            }
        }
        false
    }

    fn ensure_fragment_applicable(&self, parent_type: &str, condition: &str) -> anyhow::Result<()> {
        let parent_runtime = self.runtime_types(parent_type)?;
        if parent_runtime.is_empty() {
            bail!(
                "type `{}` cannot accept fragment spreads because it has no runtime types",
                parent_type
            );
        }

        let condition_runtime = self.runtime_types(condition)?;
        if condition_runtime.is_empty() {
            bail!(
                "type condition `{}` is not valid for fragment spreads in this schema",
                condition
            );
        }

        if parent_runtime
            .intersection(&condition_runtime)
            .next()
            .is_some()
        {
            Ok(())
        } else {
            bail!(
                "fragment type `{}` is incompatible with parent type `{}`",
                condition,
                parent_type
            )
        }
    }
}

#[derive(Clone, Debug)]
enum TypeInfo {
    Object(ObjectTypeInfo),
    Interface(InterfaceTypeInfo),
    Union(UnionTypeInfo),
    Scalar,
    Enum,
    InputObject,
}

impl TypeInfo {
    fn kind(&self) -> &'static str {
        match self {
            TypeInfo::Object(_) => "object",
            TypeInfo::Interface(_) => "interface",
            TypeInfo::Union(_) => "union",
            TypeInfo::Scalar => "scalar",
            TypeInfo::Enum => "enum",
            TypeInfo::InputObject => "input object",
        }
    }
}

#[derive(Clone, Debug)]
struct ObjectTypeInfo {
    fields: FieldCollection,
    implements: HashSet<String>,
}

impl Default for ObjectTypeInfo {
    fn default() -> Self {
        Self {
            fields: FieldCollection::default(),
            implements: HashSet::new(),
        }
    }
}

#[derive(Clone, Debug)]
struct InterfaceTypeInfo {
    fields: FieldCollection,
    implements: HashSet<String>,
}

impl Default for InterfaceTypeInfo {
    fn default() -> Self {
        Self {
            fields: FieldCollection::default(),
            implements: HashSet::new(),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct UnionTypeInfo {
    members: HashSet<String>,
}

#[derive(Clone, Debug, Default)]
struct FieldCollection {
    fields: HashMap<String, FieldInfo>,
}

impl FieldCollection {
    fn from_fields(fields: Vec<SchemaField<'static, String>>) -> Self {
        let mut collection = FieldCollection {
            fields: HashMap::with_capacity(fields.len()),
        };
        for field in fields {
            collection
                .fields
                .insert(field.name.clone(), FieldInfo::from(field));
        }
        collection
    }

    fn get(&self, name: &str) -> Option<&FieldInfo> {
        self.fields.get(name)
    }

    fn extend_fields(&mut self, fields: Vec<SchemaField<'static, String>>) {
        for field in fields {
            self.fields
                .insert(field.name.clone(), FieldInfo::from(field));
        }
    }

    fn extend_collection(&mut self, other: FieldCollection) {
        for (name, info) in other.fields {
            self.fields.insert(name, info);
        }
    }
}

#[derive(Clone, Debug)]
struct FieldInfo {
    return_type: TypeRef,
    arguments: HashMap<String, InputValueInfo>,
}

impl FieldInfo {
    fn from(field: SchemaField<'static, String>) -> Self {
        FieldInfo {
            return_type: TypeRef::from(field.field_type),
            arguments: field
                .arguments
                .into_iter()
                .map(|argument| (argument.name.clone(), InputValueInfo::from(argument)))
                .collect(),
        }
    }

    fn composite_type<'a>(&'a self, schema_index: &'a SchemaIndex) -> Option<&'a str> {
        let named = self.return_type.innermost_named()?;
        if schema_index.is_composite_type(named) {
            Some(named)
        } else {
            None
        }
    }

    fn argument(&self, name: &str) -> Option<&InputValueInfo> {
        self.arguments.get(name)
    }
}

#[derive(Clone, Debug)]
enum TypeRef {
    Named(String),
    List(Box<TypeRef>),
    NonNull(Box<TypeRef>),
}

impl TypeRef {
    fn innermost_named(&self) -> Option<&str> {
        match self {
            TypeRef::Named(name) => Some(name.as_str()),
            TypeRef::List(inner) | TypeRef::NonNull(inner) => inner.innermost_named(),
        }
    }

    fn is_non_null(&self) -> bool {
        matches!(self, TypeRef::NonNull(_))
    }
}

impl From<SchemaType<'static, String>> for TypeRef {
    fn from(value: SchemaType<'static, String>) -> Self {
        match value {
            SchemaType::NamedType(name) => TypeRef::Named(name),
            SchemaType::ListType(inner) => TypeRef::List(Box::new(TypeRef::from(*inner))),
            SchemaType::NonNullType(inner) => TypeRef::NonNull(Box::new(TypeRef::from(*inner))),
        }
    }
}

#[derive(Clone, Debug)]
struct InputValueInfo {
    type_ref: TypeRef,
    has_default: bool,
}

impl InputValueInfo {
    fn is_required(&self) -> bool {
        self.type_ref.is_non_null() && !self.has_default
    }
}

impl From<SchemaInputValue<'static, String>> for InputValueInfo {
    fn from(value: SchemaInputValue<'static, String>) -> Self {
        Self {
            type_ref: TypeRef::from(value.value_type),
            has_default: value.default_value.is_some(),
        }
    }
}

pub(crate) fn canonicalize_query(input: &str) -> String {
    dedent_and_trim(input)
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
            let value: Value = serde_json::from_str(&raw)
                .with_context(|| "failed to parse GraphQL variables_json as JSON")?;
            let canonical = serde_json::to_string(&value)?;
            Ok(Some(canonical))
        }
        None => Ok(None),
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
