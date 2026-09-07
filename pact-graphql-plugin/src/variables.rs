//! Validation of an operation's declared variable definitions against the variables actually
//! supplied.
//!
//! The schema is already in hand at configure time, so a consumer who writes
//! `{ productId: "10" }` for an operation declaring `$id: ID!` should be told at pact-write time
//! rather than discovering it when a real server rejects the request.
//!
//! Scope: variable *definitions* — presence, nullability, and input coercion of the declared type.
//! Input object interiors are checked only as far as "is it an object"; the schema index does not
//! yet carry input object fields.

use anyhow::bail;
use graphql_parser::query::OperationDefinition;
use graphql_parser::query::VariableDefinition;
use serde_json::Value;

use crate::schema_index::{SchemaIndex, TypeRef};

pub(crate) fn validate_variables(
    schema: &SchemaIndex,
    operation: &OperationDefinition<'static, String>,
    variables: Option<&Value>,
) -> anyhow::Result<()> {
    for definition in variable_definitions_of(operation) {
        let name = definition.name.as_str();
        let type_ref = TypeRef::from(definition.var_type.clone());
        let supplied = variables
            .and_then(|value| value.as_object())
            .and_then(|object| object.get(name));

        match supplied {
            None | Some(Value::Null) => {
                // A default in the document satisfies a non-null variable.
                if type_ref.is_non_null() && definition.default_value.is_none() {
                    bail!(
                        "variable `${}` is declared as `{}` but no value was supplied",
                        name,
                        render_type(&type_ref)
                    );
                }
            }
            Some(value) => validate_value(schema, &type_ref, value, name)?,
        }
    }

    // Undeclared variables are deliberately not an error: GraphQL servers ignore them.

    Ok(())
}

fn validate_value(
    schema: &SchemaIndex,
    type_ref: &TypeRef,
    value: &Value,
    name: &str,
) -> anyhow::Result<()> {
    if value.is_null() {
        if type_ref.is_non_null() {
            bail!(
                "variable `${}` does not accept null (declared `{}`)",
                name,
                render_type(type_ref)
            );
        }
        return Ok(());
    }

    if let Some(item_type) = type_ref.as_list_item() {
        let Value::Array(items) = value else {
            bail!(
                "variable `${}` is declared as `{}` but a {} was supplied",
                name,
                render_type(type_ref),
                json_kind(value)
            );
        };
        for item in items {
            validate_value(schema, item_type, item, name)?;
        }
        return Ok(());
    }

    let named = type_ref.unwrap_non_null();
    let Some(type_name) = named.innermost_named() else {
        return Ok(());
    };

    let Some(info) = schema.type_info(type_name) else {
        bail!(
            "variable `${}` is declared as `{}`, which is not a type in the schema",
            name,
            type_name
        );
    };

    let acceptable = match info {
        crate::schema_index::TypeInfo::Enum(_) => {
            let Some(member) = value.as_str() else {
                bail!(
                    "variable `${}` is declared as enum `{}` but a {} was supplied",
                    name,
                    type_name,
                    json_kind(value)
                );
            };
            match schema.enum_values(type_name) {
                Some(members) if !members.contains(member) => {
                    let mut allowed: Vec<&str> = members.iter().map(String::as_str).collect();
                    allowed.sort_unstable();
                    bail!(
                        "variable `${}` is `{}`, which is not a value of enum `{}` (expected one of: {})",
                        name,
                        member,
                        type_name,
                        allowed.join(", ")
                    );
                }
                _ => true,
            }
        }
        crate::schema_index::TypeInfo::InputObject => value.is_object(),
        crate::schema_index::TypeInfo::Scalar => scalar_accepts(type_name, value),
        other => bail!(
            "variable `${}` cannot be declared as {} type `{}`",
            name,
            other.kind(),
            type_name
        ),
    };

    if !acceptable {
        bail!(
            "variable `${}` is declared as `{}` but a {} was supplied",
            name,
            render_type(type_ref),
            json_kind(value)
        );
    }

    Ok(())
}

/// Input coercion for the built-in scalars, per the GraphQL spec's coercion rules. Custom scalars
/// accept anything: the schema says nothing about their representation.
fn scalar_accepts(type_name: &str, value: &Value) -> bool {
    match type_name {
        "Int" => value.as_i64().is_some(),
        // An Int is valid input for a Float.
        "Float" => value.is_number(),
        "String" => value.is_string(),
        // ID accepts a String or an Int.
        "ID" => value.is_string() || value.as_i64().is_some(),
        "Boolean" => value.is_boolean(),
        _ => true,
    }
}

fn variable_definitions_of<'a>(
    operation: &'a OperationDefinition<'static, String>,
) -> &'a [VariableDefinition<'static, String>] {
    match operation {
        OperationDefinition::Query(query) => &query.variable_definitions,
        OperationDefinition::Mutation(mutation) => &mutation.variable_definitions,
        OperationDefinition::Subscription(subscription) => &subscription.variable_definitions,
        OperationDefinition::SelectionSet(_) => &[],
    }
}

fn render_type(type_ref: &TypeRef) -> String {
    match type_ref {
        TypeRef::Named(name) => name.clone(),
        TypeRef::List(inner) => format!("[{}]", render_type(inner)),
        TypeRef::NonNull(inner) => format!("{}!", render_type(inner)),
    }
}

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

#[cfg(test)]
#[path = "variables_tests.rs"]
mod tests;
