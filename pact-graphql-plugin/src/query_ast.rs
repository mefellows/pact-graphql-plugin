use std::collections::HashMap;

use anyhow::{anyhow, bail, Context};
use graphql_parser::query::{
    parse_query, Definition, Document, Field as QueryField, FragmentDefinition,
    OperationDefinition, Selection, SelectionSet,
};
use serde::{Deserialize, Serialize};

/// Parses `query`, selects the requested operation, inlines every fragment
/// spread it reaches, and returns the operation as an owned AST node.
pub(crate) fn parse_and_inline(
    query: &str,
    operation_name: Option<&str>,
) -> anyhow::Result<OperationDefinition<'static, String>> {
    let document: Document<'static, String> = parse_query::<String>(query)
        .context("failed to parse GraphQL query document")?
        .into_static();

    let fragments: HashMap<String, FragmentDefinition<'static, String>> = document
        .definitions
        .iter()
        .filter_map(|definition| match definition {
            Definition::Fragment(fragment) => Some((fragment.name.clone(), fragment.clone())),
            _ => None,
        })
        .collect();

    let mut operation = select_operation(&document, operation_name)?;
    let selection_set = selection_set_mut(&mut operation);
    inline_selection_set(selection_set, &fragments, &mut Vec::new())?;

    Ok(operation)
}

/// Canonical text form of an operation: whitespace-normalised, comment-free,
/// fragment-inlined.
pub(crate) fn canonical_document(
    query: &str,
    operation_name: Option<&str>,
) -> anyhow::Result<String> {
    let operation = parse_and_inline(query, operation_name)?;
    Ok(operation.to_string().trim().to_string())
}

fn select_operation(
    document: &Document<'static, String>,
    operation_name: Option<&str>,
) -> anyhow::Result<OperationDefinition<'static, String>> {
    let operations: Vec<&OperationDefinition<'static, String>> = document
        .definitions
        .iter()
        .filter_map(|definition| match definition {
            Definition::Operation(operation) => Some(operation),
            _ => None,
        })
        .collect();

    if operations.is_empty() {
        bail!("query_document must define at least one operation");
    }

    match operation_name {
        Some(name) => operations
            .into_iter()
            .find(|operation| operation_name_of(operation) == Some(name))
            .cloned()
            .ok_or_else(|| anyhow!("operation `{}` not found in query_document", name)),
        None => {
            if operations.len() > 1 {
                bail!(
                    "operation_name is required when query_document defines multiple operations"
                );
            }
            Ok(operations[0].clone())
        }
    }
}

pub(crate) fn operation_name_of<'a>(
    operation: &'a OperationDefinition<'static, String>,
) -> Option<&'a str> {
    match operation {
        OperationDefinition::Query(query) => query.name.as_deref(),
        OperationDefinition::Mutation(mutation) => mutation.name.as_deref(),
        OperationDefinition::Subscription(subscription) => subscription.name.as_deref(),
        OperationDefinition::SelectionSet(_) => None,
    }
}

pub(crate) fn selection_set_of<'a>(
    operation: &'a OperationDefinition<'static, String>,
) -> &'a SelectionSet<'static, String> {
    match operation {
        OperationDefinition::Query(query) => &query.selection_set,
        OperationDefinition::Mutation(mutation) => &mutation.selection_set,
        OperationDefinition::Subscription(subscription) => &subscription.selection_set,
        OperationDefinition::SelectionSet(selection_set) => selection_set,
    }
}

fn selection_set_mut<'a>(
    operation: &'a mut OperationDefinition<'static, String>,
) -> &'a mut SelectionSet<'static, String> {
    match operation {
        OperationDefinition::Query(query) => &mut query.selection_set,
        OperationDefinition::Mutation(mutation) => &mut mutation.selection_set,
        OperationDefinition::Subscription(subscription) => &mut subscription.selection_set,
        OperationDefinition::SelectionSet(selection_set) => selection_set,
    }
}

/// Builds the shared "fragment forms a cycle" diagnostic. `stack` holds the
/// fragment names already being expanded (outermost first) and `next` is the
/// fragment name whose spread would re-enter the stack. Used by both
/// `inline_selection_set` here and `graphql_payload::detect_fragment_cycle` so
/// the message can only be defined in one place.
pub(crate) fn fragment_cycle_error(stack: &[&str], next: &str) -> anyhow::Error {
    let mut cycle: Vec<&str> = stack.to_vec();
    cycle.push(next);
    anyhow!("fragment `{}` forms a cycle: {}", next, cycle.join(" -> "))
}

/// Replaces every `...Name` spread with the fragment's own selections, in place.
/// Inline fragments (`... on Type`) are left alone — their type condition is
/// semantically meaningful and cannot be flattened away.
///
/// `stack` holds the fragment names currently being expanded, so a spread that
/// re-enters a fragment already on the stack is reported as a cycle rather than
/// recursing forever. The message matches `graphql_payload::detect_fragment_cycle`
/// so a cyclic document reads the same whichever path reaches it first.
fn inline_selection_set(
    selection_set: &mut SelectionSet<'static, String>,
    fragments: &HashMap<String, FragmentDefinition<'static, String>>,
    stack: &mut Vec<String>,
) -> anyhow::Result<()> {
    let mut expanded: Vec<Selection<'static, String>> =
        Vec::with_capacity(selection_set.items.len());

    for selection in selection_set.items.drain(..) {
        match selection {
            Selection::Field(mut field) => {
                inline_selection_set(&mut field.selection_set, fragments, stack)?;
                expanded.push(Selection::Field(field));
            }
            Selection::FragmentSpread(spread) => {
                if let Some(position) =
                    stack.iter().position(|name| name == &spread.fragment_name)
                {
                    let cycle_stack: Vec<&str> =
                        stack[position..].iter().map(String::as_str).collect();
                    return Err(fragment_cycle_error(
                        &cycle_stack,
                        spread.fragment_name.as_str(),
                    ));
                }

                let fragment = fragments.get(&spread.fragment_name).ok_or_else(|| {
                    anyhow!(
                        "fragment `{}` is not defined in the document",
                        spread.fragment_name
                    )
                })?;
                let mut nested = fragment.selection_set.clone();
                stack.push(spread.fragment_name.clone());
                let result = inline_selection_set(&mut nested, fragments, stack);
                stack.pop();
                result?;
                expanded.extend(nested.items);
            }
            Selection::InlineFragment(mut fragment) => {
                inline_selection_set(&mut fragment.selection_set, fragments, stack)?;
                expanded.push(Selection::InlineFragment(fragment));
            }
        }
    }

    selection_set.items = expanded;
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct QueryDiff {
    pub(crate) path: String,
    pub(crate) expected: String,
    pub(crate) actual: String,
    pub(crate) description: String,
}

impl QueryDiff {
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

fn operation_kind_label(operation: &OperationDefinition<'static, String>) -> &'static str {
    match operation {
        OperationDefinition::Query(_) | OperationDefinition::SelectionSet(_) => "query",
        OperationDefinition::Mutation(_) => "mutation",
        OperationDefinition::Subscription(_) => "subscription",
    }
}

/// The response key a field contributes: its alias when present, else its name.
fn response_key<'a>(field: &'a QueryField<'static, String>) -> &'a str {
    field.alias.as_deref().unwrap_or(field.name.as_str())
}

fn render_arguments(field: &QueryField<'static, String>) -> String {
    if field.arguments.is_empty() {
        return String::from("()");
    }
    let mut parts: Vec<String> = field
        .arguments
        .iter()
        .map(|(name, value)| format!("{name}: {value}"))
        .collect();
    // Argument order is not semantically significant in GraphQL.
    parts.sort();
    format!("({})", parts.join(", "))
}

pub(crate) fn diff_operations(
    expected: &OperationDefinition<'static, String>,
    actual: &OperationDefinition<'static, String>,
) -> Vec<QueryDiff> {
    let mut diffs = Vec::new();

    let expected_kind = operation_kind_label(expected);
    let actual_kind = operation_kind_label(actual);
    if expected_kind != actual_kind {
        diffs.push(QueryDiff::new(
            "",
            expected_kind,
            actual_kind,
            "GraphQL operation type differs",
        ));
        return diffs;
    }

    let expected_name = operation_name_of(expected).unwrap_or("");
    let actual_name = operation_name_of(actual).unwrap_or("");
    if expected_name != actual_name {
        diffs.push(QueryDiff::new(
            "",
            expected_name,
            actual_name,
            "GraphQL operation name differs",
        ));
    }

    diff_selection_sets(
        selection_set_of(expected),
        selection_set_of(actual),
        "",
        &mut diffs,
    );

    diffs
}

fn join_path(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}.{key}")
    }
}

fn diff_selection_sets(
    expected: &SelectionSet<'static, String>,
    actual: &SelectionSet<'static, String>,
    prefix: &str,
    diffs: &mut Vec<QueryDiff>,
) {
    let expected_fields = collect_fields(expected);
    let actual_fields = collect_fields(actual);

    for (key, expected_field) in &expected_fields {
        let path = join_path(prefix, key);
        match actual_fields.iter().find(|(name, _)| name == key) {
            None => diffs.push(QueryDiff::new(
                path,
                key.clone(),
                "<absent>",
                format!("field `{key}` was expected but is not selected by the actual query"),
            )),
            Some((_, actual_field)) => {
                if expected_field.name != actual_field.name {
                    diffs.push(QueryDiff::new(
                        path.clone(),
                        expected_field.name.clone(),
                        actual_field.name.clone(),
                        format!("alias `{key}` resolves to a different field"),
                    ));
                    continue;
                }

                let expected_args = render_arguments(expected_field);
                let actual_args = render_arguments(actual_field);
                if expected_args != actual_args {
                    diffs.push(QueryDiff::new(
                        path.clone(),
                        expected_args,
                        actual_args,
                        format!("field `{key}` argument values differ"),
                    ));
                }

                diff_selection_sets(
                    &expected_field.selection_set,
                    &actual_field.selection_set,
                    &path,
                    diffs,
                );
            }
        }
    }

    for (key, _) in &actual_fields {
        if !expected_fields.iter().any(|(name, _)| name == key) {
            diffs.push(QueryDiff::new(
                join_path(prefix, key),
                "<absent>",
                key.clone(),
                format!("field `{key}` is selected by the actual query but was not expected"),
            ));
        }
    }
}

/// Flattens a selection set into `(response key, field)` pairs. Inline
/// fragments are flattened into their parent — the type condition is preserved
/// in the response key so two different conditions do not collide.
fn collect_fields<'a>(
    selection_set: &'a SelectionSet<'static, String>,
) -> Vec<(String, &'a QueryField<'static, String>)> {
    let mut fields = Vec::new();
    collect_fields_into(selection_set, "", &mut fields);
    fields
}

fn collect_fields_into<'a>(
    selection_set: &'a SelectionSet<'static, String>,
    condition_prefix: &str,
    fields: &mut Vec<(String, &'a QueryField<'static, String>)>,
) {
    for selection in &selection_set.items {
        match selection {
            Selection::Field(field) => {
                let key = if condition_prefix.is_empty() {
                    response_key(field).to_string()
                } else {
                    format!("{condition_prefix}{}", response_key(field))
                };
                fields.push((key, field));
            }
            Selection::InlineFragment(fragment) => {
                let condition = match &fragment.type_condition {
                    Some(graphql_parser::query::TypeCondition::On(name)) => {
                        format!("... on {name}/")
                    }
                    None => String::new(),
                };
                collect_fields_into(&fragment.selection_set, &condition, fields);
            }
            // Spreads were inlined by `parse_and_inline` before we get here.
            Selection::FragmentSpread(_) => {}
        }
    }
}

/// Controls how strictly the actual query document is compared to the
/// expected one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryMatching {
    /// Byte-for-byte comparison of the canonical text.
    Exact,
    /// AST comparison: formatting and fragment structure are ignored.
    #[default]
    Semantic,
    /// Like `Semantic`, but the actual query may omit expected fields.
    Subset,
}

pub(crate) fn diff_exact(expected: &str, actual: &str) -> Vec<QueryDiff> {
    if expected.trim() == actual.trim() {
        return Vec::new();
    }
    vec![QueryDiff::new(
        "",
        expected.trim(),
        actual.trim(),
        "GraphQL query document differs byte-for-byte (queryMatching: exact)",
    )]
}

pub(crate) fn diff_operations_with(
    expected: &OperationDefinition<'static, String>,
    actual: &OperationDefinition<'static, String>,
    mode: QueryMatching,
) -> Vec<QueryDiff> {
    let diffs = diff_operations(expected, actual);
    match mode {
        QueryMatching::Subset => diffs
            .into_iter()
            .filter(|diff| !diff.description.contains("is not selected by the actual query"))
            .collect(),
        _ => diffs,
    }
}

#[cfg(test)]
#[path = "query_ast_tests.rs"]
mod tests;
