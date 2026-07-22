use std::collections::HashMap;

use anyhow::{anyhow, bail, Context};
use graphql_parser::query::{
    parse_query, Definition, Document, FragmentDefinition, OperationDefinition, Selection,
    SelectionSet,
};

const MAX_FRAGMENT_DEPTH: usize = 64;

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
    inline_selection_set(selection_set, &fragments, 0)?;

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

// Consumed by Task 4's `diff_operations`; not yet called within Task 3.
#[allow(dead_code)]
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

/// Replaces every `...Name` spread with the fragment's own selections, in place.
/// Inline fragments (`... on Type`) are left alone — their type condition is
/// semantically meaningful and cannot be flattened away.
fn inline_selection_set(
    selection_set: &mut SelectionSet<'static, String>,
    fragments: &HashMap<String, FragmentDefinition<'static, String>>,
    depth: usize,
) -> anyhow::Result<()> {
    if depth > MAX_FRAGMENT_DEPTH {
        bail!("fragment spreads nested more than {MAX_FRAGMENT_DEPTH} deep; possible cycle");
    }

    let mut expanded: Vec<Selection<'static, String>> =
        Vec::with_capacity(selection_set.items.len());

    for selection in selection_set.items.drain(..) {
        match selection {
            Selection::Field(mut field) => {
                inline_selection_set(&mut field.selection_set, fragments, depth + 1)?;
                expanded.push(Selection::Field(field));
            }
            Selection::FragmentSpread(spread) => {
                let fragment = fragments.get(&spread.fragment_name).ok_or_else(|| {
                    anyhow!(
                        "fragment `{}` is not defined in the document",
                        spread.fragment_name
                    )
                })?;
                let mut nested = fragment.selection_set.clone();
                inline_selection_set(&mut nested, fragments, depth + 1)?;
                expanded.extend(nested.items);
            }
            Selection::InlineFragment(mut fragment) => {
                inline_selection_set(&mut fragment.selection_set, fragments, depth + 1)?;
                expanded.push(Selection::InlineFragment(fragment));
            }
        }
    }

    selection_set.items = expanded;
    Ok(())
}

#[cfg(test)]
#[path = "query_ast_tests.rs"]
mod tests;
