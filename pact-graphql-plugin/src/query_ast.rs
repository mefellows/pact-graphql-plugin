use std::collections::HashMap;

use anyhow::{anyhow, bail, Context};
use graphql_parser::query::{
    parse_query, Definition, Document, FragmentDefinition, OperationDefinition, Selection,
    SelectionSet,
};

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

// TODO(task-4): remove this allow once diff_operations calls this.
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
                    let mut cycle: Vec<&str> =
                        stack[position..].iter().map(String::as_str).collect();
                    cycle.push(spread.fragment_name.as_str());
                    bail!(
                        "fragment `{}` forms a cycle: {}",
                        spread.fragment_name,
                        cycle.join(" -> ")
                    );
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

#[cfg(test)]
#[path = "query_ast_tests.rs"]
mod tests;
