# GraphQL Core Semantics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Rust plugin compare GraphQL queries semantically (on the AST, not as strings) and validate GraphQL *responses* against the schema and the query's selection set, emitting schema-derived matching rules.

**Architecture:** Extract the existing schema model out of the 1550-line `graphql_payload.rs` into its own module and extend it with the type information response validation needs (enum members, field return types). Add two new modules: `query_ast` (canonical AST printing + structural diff with field paths) and `response` (envelope validation + matcher derivation). Register a second content type, `application/graphql-response`, so the plugin can tell request bodies from response bodies in `compare_contents`, and return a second `InteractionResponse` with `partName: "response"` from `configure_interaction`.

**Tech Stack:** Rust 2021, `graphql-parser` 0.4, `serde_json` 1 (with `preserve_order`), `anyhow` 1, `tonic` 0.13, `pact-plugin-driver` 0.7.5. Tests use `pretty_assertions` and the built-in test harness.

## Global Constraints

- Crate name is `pact_graphql_plugin`. All `cargo` commands run from the repo root; the crate lives at `pact-graphql-plugin/`.
- Rust edition 2021. Do not add new dependencies — everything needed is already in `pact-graphql-plugin/Cargo.toml`.
- `graphql-parser` 0.4 is the only GraphQL parser. Do not introduce another.
- All new public-ish items are `pub(crate)` unless a task explicitly says otherwise. Only `lib.rs` re-exports are `pub`.
- Existing tests in `pact-graphql-plugin/tests/` must keep passing at every commit. Run `cargo test` before every commit.
- Mismatch `path` values use JSON-path style with a `$` root (e.g. `$.data.product.id`) for responses, and `/`-prefixed pointers (e.g. `/payload/query_document`) for requests. Do not mix the two conventions.
- Commit messages follow Conventional Commits (`feat:`, `refactor:`, `test:`, `fix:`).
- Never use `unwrap()` or `expect()` on anything derived from user input. Return `anyhow::Result` or push a mismatch.

---

## File Structure

**Created:**
- `pact-graphql-plugin/src/schema_index.rs` — the schema model: `SchemaIndex`, `TypeInfo`, `FieldInfo`, `TypeRef`, `FieldCollection`, and the SDL registration logic. Moved verbatim from `graphql_payload.rs`, then extended.
- `pact-graphql-plugin/src/query_ast.rs` — canonical AST printing, fragment inlining, structural document diff.
- `pact-graphql-plugin/src/response.rs` — GraphQL response envelope parsing, validation against a selection set, schema-derived matching rules.
- `pact-graphql-plugin/tests/query_ast_tests.rs` — integration tests for `query_ast`.
- `pact-graphql-plugin/tests/response_tests.rs` — integration tests for `response`.

**Modified:**
- `pact-graphql-plugin/src/lib.rs` — declare the three new modules.
- `pact-graphql-plugin/src/graphql_payload.rs` — delete the moved schema code; import from `schema_index`; replace the string-equality query diff with the AST diff.
- `pact-graphql-plugin/src/interaction.rs` — add `query_matching` and `response_body_json` to the config structs.
- `pact-graphql-plugin/src/server.rs` — catalogue entry for the response content type; response branch in `compare_contents`; second `InteractionResponse` from `configure_interaction`.

---

### Task 1: Extract the schema model into its own module

Pure refactor. No behaviour change. This exists so Tasks 2, 6 and 7 have a file they can hold in context.

**Files:**
- Create: `pact-graphql-plugin/src/schema_index.rs`
- Modify: `pact-graphql-plugin/src/lib.rs:1-5`
- Modify: `pact-graphql-plugin/src/graphql_payload.rs` (delete lines ~870-1460, add imports)

**Interfaces:**
- Consumes: nothing.
- Produces: module `crate::schema_index` exporting `pub(crate) struct SchemaIndex` with `pub(crate) fn from_sdl(sdl: &str) -> anyhow::Result<SchemaIndex>`, `pub(crate) fn root_type(&self, kind: OperationKind) -> Option<&str>`, `pub(crate) fn type_info(&self, name: &str) -> Option<&TypeInfo>`, `pub(crate) fn is_composite_type(&self, name: &str) -> bool`, `pub(crate) fn is_union_type(&self, name: &str) -> bool`, `pub(crate) fn is_query_root(&self, name: &str) -> bool`, `pub(crate) fn ensure_type_exists(&self, name: &str) -> anyhow::Result<()>`, `pub(crate) fn field<'a>(&'a self, parent: &'a TypeInfo, name: &str) -> Option<&'a FieldInfo>`, `pub(crate) fn runtime_types(&self, name: &str) -> anyhow::Result<HashSet<String>>`, `pub(crate) fn ensure_fragment_applicable(&self, parent: &str, condition: &str) -> anyhow::Result<()>`. Also exports `pub(crate) enum OperationKind { Query, Mutation, Subscription }`, `pub(crate) enum TypeInfo`, `pub(crate) struct FieldInfo`, `pub(crate) enum TypeRef`, `pub(crate) struct InputValueInfo`, and `pub(crate) const INTROSPECTION_SDL: &str`.

- [ ] **Step 1: Create the new module file with the moved code**

Create `pact-graphql-plugin/src/schema_index.rs`. Move these items **verbatim** out of `pact-graphql-plugin/src/graphql_payload.rs`:

- `const INTROSPECTION_SDL` (currently line 387)
- `enum OperationKind` (line 559)
- `struct SchemaIndex` and its entire `impl` block (the `from_sdl`, `register_document`, `apply_schema_definition`, `register_type`, `apply_type_extension`, `root_type`, `type_info`, `is_composite_type`, `is_union_type`, `is_query_root`, `ensure_type_exists`, `field`, `runtime_types`, `interface_runtime_types`, `object_implements_interface`, `ensure_fragment_applicable` methods)
- `enum TypeInfo` and its `impl`
- `struct ObjectTypeInfo`, `struct InterfaceTypeInfo`, `struct UnionTypeInfo` and their `Default` impls
- `struct FieldCollection` and its `impl`
- `struct FieldInfo` and its `impl`
- `enum TypeRef`, its `impl`, and its `From<SchemaType<'static, String>>` impl
- `struct InputValueInfo`, its `impl`, and its `From<SchemaInputValue<'static, String>>` impl

Change every one of the following from private to `pub(crate)`: `INTROSPECTION_SDL`, `OperationKind`, `SchemaIndex`, `TypeInfo`, `ObjectTypeInfo`, `InterfaceTypeInfo`, `UnionTypeInfo`, `FieldCollection`, `FieldInfo`, `TypeRef`, `InputValueInfo`, and the `SchemaIndex` methods listed in the Interfaces block above, plus `TypeInfo::kind`, `FieldInfo::composite_type`, `FieldInfo::argument`, `TypeRef::innermost_named`, `TypeRef::is_non_null`, `InputValueInfo::is_required`, `FieldCollection::get`.

The file must start with these imports:

```rust
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use anyhow::{bail, Context};
use graphql_parser::schema::{
    parse_schema, Definition as SchemaDefinition, Document as SchemaDocument, Field as SchemaField,
    InputValue as SchemaInputValue, Type as SchemaType, TypeDefinition, TypeExtension,
};
```

- [ ] **Step 2: Declare the module**

In `pact-graphql-plugin/src/lib.rs`, add the module declaration in alphabetical position:

```rust
pub mod encoder;
pub mod graphql_payload;
pub mod interaction;
pub mod schema;
pub(crate) mod schema_index;
pub mod server;
```

- [ ] **Step 3: Delete the moved code from `graphql_payload.rs` and import it**

Remove every item listed in Step 1 from `pact-graphql-plugin/src/graphql_payload.rs`. Add this import near the other `use crate::` lines at the top of the file:

```rust
use crate::schema_index::{FieldInfo, OperationKind, SchemaIndex, TypeInfo};
```

Then prune the now-unused imports from `graphql_payload.rs`'s header: remove `std::collections::hash_map::Entry`, and remove `Field as SchemaField`, `InputValue as SchemaInputValue`, `Type as SchemaType`, `TypeDefinition`, `TypeExtension`, `Definition as SchemaDefinition`, `Document as SchemaDocument`, `parse_schema` from the `graphql_parser::schema` import list. Keep `HashMap` and `HashSet` — they are still used by `FragmentMap` and the fragment cycle logic.

- [ ] **Step 4: Verify the refactor compiles and changes nothing**

Run: `cargo test`
Expected: PASS. Every existing test in `tests/schema_tests.rs`, `tests/encoder_tests.rs`, `tests/interaction_tests.rs`, `tests/plugin_flow.rs` and the inline `#[cfg(test)]` modules passes, with zero warnings about unused imports.

If you get `warning: unused import`, delete that import. Do not add `#[allow(unused)]`.

- [ ] **Step 5: Commit**

```bash
git add pact-graphql-plugin/src/schema_index.rs pact-graphql-plugin/src/lib.rs pact-graphql-plugin/src/graphql_payload.rs
git commit -m "refactor: extract schema model into schema_index module"
```

---

### Task 2: Teach the schema model about enum values and response-side type queries

Response validation needs three things the model does not currently carry: the members of an enum (`TypeInfo::Enum` is a unit variant today), a way to look up a field's return type from a type name and field name, and a way to unwrap a `TypeRef`.

**Files:**
- Modify: `pact-graphql-plugin/src/schema_index.rs`
- Test: `pact-graphql-plugin/tests/schema_index_tests.rs` (create)

**Interfaces:**
- Consumes: `SchemaIndex`, `TypeInfo`, `TypeRef`, `FieldInfo` from Task 1.
- Produces:
  - `TypeInfo::Enum(EnumTypeInfo)` where `pub(crate) struct EnumTypeInfo { pub(crate) values: HashSet<String> }`
  - `SchemaIndex::enum_values(&self, type_name: &str) -> Option<&HashSet<String>>`
  - `SchemaIndex::field_return_type(&self, parent_type: &str, field_name: &str) -> Option<&TypeRef>`
  - `SchemaIndex::is_enum(&self, type_name: &str) -> bool`
  - `SchemaIndex::is_scalar(&self, type_name: &str) -> bool`
  - `TypeRef::unwrap_non_null(&self) -> &TypeRef`
  - `TypeRef::as_list_item(&self) -> Option<&TypeRef>`

- [ ] **Step 1: Write the failing test**

Create `pact-graphql-plugin/tests/schema_index_tests.rs`:

```rust
use pact_graphql_plugin::test_support::schema_index_for;

const SDL: &str = r#"
type Query { product(id: ID!): Product }
type Product { id: ID!, name: String, status: ProductStatus!, tags: [String!]! }
enum ProductStatus { ACTIVE ARCHIVED }
"#;

#[test]
fn exposes_enum_values() {
    let index = schema_index_for(SDL).expect("schema parses");
    let values = index.enum_values("ProductStatus").expect("enum is known");
    assert!(values.contains("ACTIVE"));
    assert!(values.contains("ARCHIVED"));
    assert_eq!(values.len(), 2);
}

#[test]
fn unknown_enum_returns_none() {
    let index = schema_index_for(SDL).expect("schema parses");
    assert!(index.enum_values("Product").is_none());
    assert!(index.enum_values("Nope").is_none());
}

#[test]
fn resolves_field_return_types() {
    let index = schema_index_for(SDL).expect("schema parses");

    let id = index.field_return_type("Product", "id").expect("id exists");
    assert!(id.is_non_null());
    assert_eq!(id.innermost_named(), Some("ID"));

    let name = index.field_return_type("Product", "name").expect("name exists");
    assert!(!name.is_non_null());

    assert!(index.field_return_type("Product", "nope").is_none());
}

#[test]
fn unwraps_list_and_non_null() {
    let index = schema_index_for(SDL).expect("schema parses");
    let tags = index.field_return_type("Product", "tags").expect("tags exists");

    // [String!]! -> NonNull(List(NonNull(String)))
    assert!(tags.is_non_null());
    let list = tags.unwrap_non_null();
    let item = list.as_list_item().expect("is a list");
    assert!(item.is_non_null());
    assert_eq!(item.unwrap_non_null().innermost_named(), Some("String"));
}

#[test]
fn classifies_enums_and_scalars() {
    let index = schema_index_for(SDL).expect("schema parses");
    assert!(index.is_enum("ProductStatus"));
    assert!(!index.is_enum("Product"));
    assert!(index.is_scalar("ID"));
    assert!(index.is_scalar("String"));
    assert!(!index.is_scalar("Product"));
}
```

- [ ] **Step 2: Add the test-support seam**

The tests above need to construct a `SchemaIndex` from outside the crate, but `SchemaIndex` is `pub(crate)`. Add a gated public seam rather than widening the type's visibility.

In `pact-graphql-plugin/src/lib.rs`, append:

```rust
#[doc(hidden)]
pub mod test_support {
    //! Test-only seam. Not part of the supported API.
    pub use crate::schema_index::{SchemaIndex, TypeRef};

    pub fn schema_index_for(sdl: &str) -> anyhow::Result<SchemaIndex> {
        SchemaIndex::from_sdl(sdl)
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test --test schema_index_tests`
Expected: FAIL to compile, with errors including `no method named 'enum_values' found for struct 'SchemaIndex'` and `no method named 'unwrap_non_null' found for enum 'TypeRef'`.

- [ ] **Step 4: Give `TypeInfo::Enum` a payload**

In `pact-graphql-plugin/src/schema_index.rs`, replace the unit `Enum` variant:

```rust
#[derive(Clone, Debug, Default)]
pub(crate) struct EnumTypeInfo {
    pub(crate) values: HashSet<String>,
}
```

Change the variant in `enum TypeInfo` from `Enum,` to `Enum(EnumTypeInfo),`.

In `TypeInfo::kind`, change the arm to `TypeInfo::Enum(_) => "enum",`.

In `register_type`, replace the whole `TypeDefinition::Enum(enum_type)` arm with:

```rust
TypeDefinition::Enum(enum_type) => {
    let values: HashSet<String> = enum_type
        .values
        .into_iter()
        .map(|value| value.name)
        .collect();
    match self.types.entry(enum_type.name.clone()) {
        Entry::Vacant(entry) => {
            entry.insert(TypeInfo::Enum(EnumTypeInfo { values }));
        }
        Entry::Occupied(mut entry) => match entry.get_mut() {
            TypeInfo::Enum(existing) => {
                existing.values.extend(values);
            }
            other => {
                bail!(
                    "type `{}` redeclared as enum but previously defined as {}",
                    enum_type.name,
                    other.kind()
                );
            }
        },
    }
}
```

In `apply_type_extension`, find the `TypeExtension::Enum` arm and make it merge values the same way — `or_insert_with(|| TypeInfo::Enum(EnumTypeInfo::default()))`, then `existing.values.extend(ext.values.into_iter().map(|v| v.name))`.

In `runtime_types`, change the pattern `Some(TypeInfo::Scalar) | Some(TypeInfo::Enum) | Some(TypeInfo::InputObject)` to `Some(TypeInfo::Scalar) | Some(TypeInfo::Enum(_)) | Some(TypeInfo::InputObject)`.

Then run `cargo build` and fix any remaining `TypeInfo::Enum` pattern matches the compiler flags — there may be others in `graphql_payload.rs`.

- [ ] **Step 5: Add the new accessors**

Append these methods inside the existing `impl SchemaIndex` block in `pact-graphql-plugin/src/schema_index.rs`:

```rust
pub(crate) fn enum_values(&self, type_name: &str) -> Option<&HashSet<String>> {
    match self.types.get(type_name) {
        Some(TypeInfo::Enum(info)) => Some(&info.values),
        _ => None,
    }
}

pub(crate) fn is_enum(&self, type_name: &str) -> bool {
    matches!(self.types.get(type_name), Some(TypeInfo::Enum(_)))
}

pub(crate) fn is_scalar(&self, type_name: &str) -> bool {
    matches!(self.types.get(type_name), Some(TypeInfo::Scalar))
}

pub(crate) fn field_return_type(
    &self,
    parent_type: &str,
    field_name: &str,
) -> Option<&TypeRef> {
    let parent = self.types.get(parent_type)?;
    self.field(parent, field_name).map(|info| &info.return_type)
}
```

`field_return_type` borrows `FieldInfo::return_type`, so make that field `pub(crate)`.

Append to the existing `impl TypeRef` block:

```rust
/// Strips a single `NonNull` wrapper, if present.
pub(crate) fn unwrap_non_null(&self) -> &TypeRef {
    match self {
        TypeRef::NonNull(inner) => inner,
        other => other,
    }
}

/// Returns the item type when this is a list, ignoring any outer `NonNull`.
pub(crate) fn as_list_item(&self) -> Option<&TypeRef> {
    match self.unwrap_non_null() {
        TypeRef::List(inner) => Some(inner),
        _ => None,
    }
}
```

Note: the built-in scalars `ID`, `String`, `Int`, `Float` and `Boolean` are not declared in `INTROSPECTION_SDL`, so `is_scalar("String")` would return `false`. Fix this by appending the built-in scalar declarations to `INTROSPECTION_SDL`:

```
scalar ID
scalar String
scalar Int
scalar Float
scalar Boolean
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test --test schema_index_tests`
Expected: PASS — all 5 tests.

Run: `cargo test`
Expected: PASS — the full suite, including everything from Task 1.

- [ ] **Step 7: Commit**

```bash
git add pact-graphql-plugin/src/schema_index.rs pact-graphql-plugin/src/lib.rs pact-graphql-plugin/tests/schema_index_tests.rs pact-graphql-plugin/src/graphql_payload.rs
git commit -m "feat: carry enum members and field return types in the schema index"
```

---

### Task 3: Canonical AST printing for query documents

Today `canonicalize_query` only dedents and trims, so a comment or a different inline spacing produces a false mismatch. Replace it with a parse-and-reprint that also inlines fragment spreads, so a codegen client that inlines fragments matches a hand-written document.

**Files:**
- Create: `pact-graphql-plugin/src/query_ast.rs`
- Modify: `pact-graphql-plugin/src/lib.rs`
- Test: `pact-graphql-plugin/tests/query_ast_tests.rs` (create)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `pub(crate) fn canonical_document(query: &str, operation_name: Option<&str>) -> anyhow::Result<String>` — parses, inlines fragment spreads into the selected operation, and returns the `graphql-parser` pretty-print. Also `pub(crate) fn parse_and_inline(query: &str, operation_name: Option<&str>) -> anyhow::Result<OperationDefinition<'static, String>>` used by Task 4 and Task 6.

- [ ] **Step 1: Write the failing test**

Create `pact-graphql-plugin/tests/query_ast_tests.rs`:

```rust
use pact_graphql_plugin::test_support::canonical_document;

#[test]
fn ignores_insignificant_whitespace() {
    let a = canonical_document("query Q { product(id: $id) { id name } }", Some("Q")).unwrap();
    let b = canonical_document(
        "query Q {\n  product( id : $id ) {\n    id\n    name\n  }\n}",
        Some("Q"),
    )
    .unwrap();
    assert_eq!(a, b);
}

#[test]
fn ignores_comments() {
    let a = canonical_document("query Q { product { id } }", Some("Q")).unwrap();
    let b = canonical_document(
        "# fetch the product\nquery Q {\n  product { id } # just the id\n}",
        Some("Q"),
    )
    .unwrap();
    assert_eq!(a, b);
}

#[test]
fn inlines_fragment_spreads() {
    let with_fragment = canonical_document(
        "query Q { product { ...Fields } }\nfragment Fields on Product { id name }",
        Some("Q"),
    )
    .unwrap();
    let inlined = canonical_document("query Q { product { id name } }", Some("Q")).unwrap();
    assert_eq!(with_fragment, inlined);
}

#[test]
fn preserves_field_order() {
    let a = canonical_document("query Q { product { id name } }", Some("Q")).unwrap();
    let b = canonical_document("query Q { product { name id } }", Some("Q")).unwrap();
    assert_ne!(a, b, "field order is significant in GraphQL responses");
}

#[test]
fn preserves_aliases_and_arguments() {
    let out = canonical_document(
        "query Q { mine: product(id: \"10\", first: 5) { id } }",
        Some("Q"),
    )
    .unwrap();
    assert!(out.contains("mine: product"), "alias survives: {out}");
    assert!(out.contains("id: \"10\""), "arguments survive: {out}");
}

#[test]
fn rejects_an_unknown_operation_name() {
    let err = canonical_document("query Q { product { id } }", Some("Other")).unwrap_err();
    assert!(
        err.to_string().contains("Other"),
        "error names the missing operation: {err}"
    );
}

#[test]
fn rejects_an_unresolvable_fragment() {
    let err = canonical_document("query Q { product { ...Missing } }", Some("Q")).unwrap_err();
    assert!(
        err.to_string().contains("Missing"),
        "error names the missing fragment: {err}"
    );
}
```

Add to the `test_support` module in `pact-graphql-plugin/src/lib.rs`:

```rust
    pub fn canonical_document(
        query: &str,
        operation_name: Option<&str>,
    ) -> anyhow::Result<String> {
        crate::query_ast::canonical_document(query, operation_name)
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test query_ast_tests`
Expected: FAIL to compile with `failed to resolve: could not find 'query_ast' in the crate root`.

- [ ] **Step 3: Write the implementation**

Create `pact-graphql-plugin/src/query_ast.rs`:

```rust
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
            Definition::Fragment(fragment) => {
                Some((fragment.name.clone(), fragment.clone()))
            }
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
                    anyhow!("fragment `{}` is not defined in the document", spread.fragment_name)
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
```

Declare the module in `pact-graphql-plugin/src/lib.rs`:

```rust
pub(crate) mod query_ast;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --test query_ast_tests`
Expected: PASS — all 7 tests.

If `rejects_an_unresolvable_fragment` fails because the parse succeeds but no error is raised, confirm the spread is actually reached by `inline_selection_set` — it must be nested inside `product`'s selection set, and `inline_selection_set` must recurse into `Selection::Field`.

- [ ] **Step 5: Commit**

```bash
git add pact-graphql-plugin/src/query_ast.rs pact-graphql-plugin/src/lib.rs pact-graphql-plugin/tests/query_ast_tests.rs
git commit -m "feat: canonicalise query documents through the AST"
```

---

### Task 4: Structural query diff with field paths

`CanonicalGraphqlRequest::diff` currently dumps two whole documents into `expected`/`actual` on any difference. Replace it with a walk that reports the specific field path.

**Files:**
- Modify: `pact-graphql-plugin/src/query_ast.rs`
- Modify: `pact-graphql-plugin/src/graphql_payload.rs:175-226` (the `diff` method)
- Test: `pact-graphql-plugin/tests/query_ast_tests.rs` (append)

**Interfaces:**
- Consumes: `parse_and_inline`, `selection_set_of`, `operation_name_of` from Task 3.
- Produces: `pub(crate) struct QueryDiff { pub(crate) path: String, pub(crate) expected: String, pub(crate) actual: String, pub(crate) description: String }` and `pub(crate) fn diff_operations(expected: &OperationDefinition<'static, String>, actual: &OperationDefinition<'static, String>) -> Vec<QueryDiff>`.

- [ ] **Step 1: Write the failing test**

Append to `pact-graphql-plugin/tests/query_ast_tests.rs`:

```rust
use pact_graphql_plugin::test_support::diff_queries;

#[test]
fn identical_queries_produce_no_diff() {
    let diffs = diff_queries(
        "query Q { product { id name } }",
        "query Q {\n  product {\n    id\n    name\n  }\n}",
        Some("Q"),
    )
    .unwrap();
    assert!(diffs.is_empty(), "expected no diffs, got {diffs:#?}");
}

#[test]
fn reports_the_path_of_a_missing_field() {
    let diffs = diff_queries(
        "query Q { product { id name status } }",
        "query Q { product { id name } }",
        Some("Q"),
    )
    .unwrap();
    assert_eq!(diffs.len(), 1, "got {diffs:#?}");
    assert_eq!(diffs[0].path, "product.status");
    assert!(diffs[0].description.contains("not selected"), "{}", diffs[0].description);
}

#[test]
fn reports_the_path_of_an_unexpected_field() {
    let diffs = diff_queries(
        "query Q { product { id } }",
        "query Q { product { id name } }",
        Some("Q"),
    )
    .unwrap();
    assert_eq!(diffs.len(), 1, "got {diffs:#?}");
    assert_eq!(diffs[0].path, "product.name");
    assert!(diffs[0].description.contains("not expected"), "{}", diffs[0].description);
}

#[test]
fn reports_nested_paths() {
    let diffs = diff_queries(
        "query Q { product { category { id name } } }",
        "query Q { product { category { id } } }",
        Some("Q"),
    )
    .unwrap();
    assert_eq!(diffs.len(), 1, "got {diffs:#?}");
    assert_eq!(diffs[0].path, "product.category.name");
}

#[test]
fn reports_argument_differences() {
    let diffs = diff_queries(
        "query Q { product(id: \"10\") { id } }",
        "query Q { product(id: \"11\") { id } }",
        Some("Q"),
    )
    .unwrap();
    assert_eq!(diffs.len(), 1, "got {diffs:#?}");
    assert_eq!(diffs[0].path, "product");
    assert!(diffs[0].description.contains("argument"), "{}", diffs[0].description);
    assert!(diffs[0].expected.contains("10"));
    assert!(diffs[0].actual.contains("11"));
}

#[test]
fn reports_operation_type_differences() {
    let diffs = diff_queries(
        "query Q { thing { id } }",
        "mutation Q { thing { id } }",
        Some("Q"),
    )
    .unwrap();
    assert!(
        diffs.iter().any(|d| d.description.contains("operation type")),
        "got {diffs:#?}"
    );
}
```

Add to `test_support` in `pact-graphql-plugin/src/lib.rs`:

```rust
    pub use crate::query_ast::QueryDiff;

    pub fn diff_queries(
        expected: &str,
        actual: &str,
        operation_name: Option<&str>,
    ) -> anyhow::Result<Vec<QueryDiff>> {
        let expected = crate::query_ast::parse_and_inline(expected, operation_name)?;
        let actual = crate::query_ast::parse_and_inline(actual, operation_name)?;
        Ok(crate::query_ast::diff_operations(&expected, &actual))
    }
```

`QueryDiff` must derive `Debug` and `Clone` for the test assertions to compile.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test query_ast_tests`
Expected: FAIL to compile with `cannot find function 'diff_queries'` / `could not find 'QueryDiff'`.

- [ ] **Step 3: Write the implementation**

Append to `pact-graphql-plugin/src/query_ast.rs`:

```rust
use graphql_parser::query::{Field as QueryField, Value as QueryValue};

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
fn response_key(field: &QueryField<'static, String>) -> &str {
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
fn collect_fields(
    selection_set: &SelectionSet<'static, String>,
) -> Vec<(String, &QueryField<'static, String>)> {
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
```

The `QueryValue` import is needed for `render_arguments`'s `{value}` formatting to resolve; if the compiler reports it as unused because `Display` comes from the tuple element's own type, delete the import.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --test query_ast_tests`
Expected: PASS — all 13 tests (7 from Task 3, 6 new).

- [ ] **Step 5: Wire the AST diff into the request comparison**

In `pact-graphql-plugin/src/graphql_payload.rs`, replace the `query_document` block at the top of `CanonicalGraphqlRequest::diff` (currently lines 178-185) with:

```rust
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
        for diff in crate::query_ast::diff_operations(&expected_op, &actual_op) {
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
```

- [ ] **Step 6: Update the existing tests that assert on the old message**

Run: `cargo test`
Expected: some tests in `pact-graphql-plugin/tests/interaction_tests.rs` and the inline `#[cfg(test)]` module in `server.rs` will FAIL, because they assert on the description `"GraphQL query document differs"` or on the whole-document `expected`/`actual` values.

For each failure, update the assertion to the new field-path form. For example, a test that expected a whole-document mismatch for a query missing `category` now expects `path == "/payload/query_document/products/category"` and a description containing `"not selected"`. Do **not** weaken the assertions to `assert!(!mismatches.is_empty())` — assert on the specific path.

Also update `examples/js/product-consumer/pact.test.ts:445`, which asserts `rejects.toThrow(/GraphQL query document differs/)`. Change it to `/is not selected by the actual query/`.

- [ ] **Step 7: Run the full suite**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add pact-graphql-plugin/src/query_ast.rs pact-graphql-plugin/src/graphql_payload.rs pact-graphql-plugin/src/lib.rs pact-graphql-plugin/tests/ examples/js/product-consumer/pact.test.ts
git commit -m "feat: report query mismatches at the field level"
```

---

### Task 5: Query matching modes

`semantic` (the Task 4 behaviour) is the right default, but a codegen client that trims unused fields needs `subset`, and someone debugging needs `exact`.

**Files:**
- Modify: `pact-graphql-plugin/src/query_ast.rs`
- Modify: `pact-graphql-plugin/src/interaction.rs:129-149` (`GraphqlPluginRequest`), `:12-35` (`GraphqlPluginConfig` and its wire struct)
- Modify: `pact-graphql-plugin/src/graphql_payload.rs` (thread the mode into `diff`)
- Test: `pact-graphql-plugin/tests/query_ast_tests.rs` (append)

**Interfaces:**
- Consumes: `diff_operations`, `QueryDiff` from Task 4.
- Produces: `pub(crate) enum QueryMatching { Exact, Semantic, Subset }` with `Default = Semantic`, serialised as `"exact" | "semantic" | "subset"`; `pub(crate) fn diff_operations_with(expected, actual, mode: QueryMatching) -> Vec<QueryDiff>`. `GraphqlPluginRequest` and `GraphqlPluginConfig` both gain a `query_matching: QueryMatching` field.

- [ ] **Step 1: Write the failing test**

Append to `pact-graphql-plugin/tests/query_ast_tests.rs`:

```rust
use pact_graphql_plugin::test_support::{diff_queries_with, QueryMatching};

const EXPECTED: &str = "query Q { product { id name status } }";
const FEWER: &str = "query Q { product { id name } }";
const MORE: &str = "query Q { product { id name status category { id } } }";

#[test]
fn subset_allows_the_actual_query_to_request_fewer_fields() {
    let diffs = diff_queries_with(EXPECTED, FEWER, Some("Q"), QueryMatching::Subset).unwrap();
    assert!(diffs.is_empty(), "got {diffs:#?}");
}

#[test]
fn subset_still_rejects_unexpected_fields() {
    let diffs = diff_queries_with(EXPECTED, MORE, Some("Q"), QueryMatching::Subset).unwrap();
    assert_eq!(diffs.len(), 1, "got {diffs:#?}");
    assert_eq!(diffs[0].path, "product.category");
}

#[test]
fn semantic_rejects_fewer_fields() {
    let diffs = diff_queries_with(EXPECTED, FEWER, Some("Q"), QueryMatching::Semantic).unwrap();
    assert_eq!(diffs.len(), 1, "got {diffs:#?}");
    assert_eq!(diffs[0].path, "product.status");
}

#[test]
fn exact_rejects_a_reformatted_but_equivalent_query() {
    let reformatted = "query Q {\n  product {\n    id\n    name\n    status\n  }\n}";
    let semantic =
        diff_queries_with(EXPECTED, reformatted, Some("Q"), QueryMatching::Semantic).unwrap();
    assert!(semantic.is_empty(), "semantic tolerates formatting: {semantic:#?}");

    let exact = diff_queries_with(EXPECTED, reformatted, Some("Q"), QueryMatching::Exact).unwrap();
    assert_eq!(exact.len(), 1, "exact does not: {exact:#?}");
    assert!(exact[0].description.contains("byte-for-byte"), "{}", exact[0].description);
}
```

Add to `test_support` in `pact-graphql-plugin/src/lib.rs`:

```rust
    pub use crate::query_ast::QueryMatching;

    pub fn diff_queries_with(
        expected: &str,
        actual: &str,
        operation_name: Option<&str>,
        mode: QueryMatching,
    ) -> anyhow::Result<Vec<QueryDiff>> {
        if mode == QueryMatching::Exact {
            return Ok(crate::query_ast::diff_exact(expected, actual));
        }
        let expected_op = crate::query_ast::parse_and_inline(expected, operation_name)?;
        let actual_op = crate::query_ast::parse_and_inline(actual, operation_name)?;
        Ok(crate::query_ast::diff_operations_with(&expected_op, &actual_op, mode))
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test query_ast_tests`
Expected: FAIL to compile with `could not find 'QueryMatching'`.

- [ ] **Step 3: Write the implementation**

Append to `pact-graphql-plugin/src/query_ast.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum QueryMatching {
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
```

`QueryDiff::new` is private to the module today; leave it private — `diff_exact` is in the same module.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --test query_ast_tests`
Expected: PASS — all 17 tests.

- [ ] **Step 5: Thread the mode through the config**

In `pact-graphql-plugin/src/interaction.rs`:

Add to `GraphqlPluginRequest` (after `schema_sdl`):

```rust
    #[serde(default)]
    pub query_matching: crate::query_ast::QueryMatching,
```

and to its `Default` impl: `query_matching: crate::query_ast::QueryMatching::default(),`.

Add the same field to `GraphqlPluginConfig` and to `GraphqlPluginConfigWire` (as `Option<QueryMatching>` with `#[serde(default)]`), defaulting to `QueryMatching::default()` in the `Deserialize` impl and set from `config.query_matching` in the `From<&GraphqlPluginConfig>` impl.

In `GraphqlInteractionBuilder::build`, carry `req.query_matching` into the returned `GraphqlPluginConfig`. Note `build` currently destructures `req` inside `CanonicalGraphqlRequest::from_interaction_config`, so capture `let query_matching = req.query_matching;` before that call.

In `pact-graphql-plugin/src/graphql_payload.rs`, add a `query_matching: QueryMatching` field to `CanonicalGraphqlRequest`, populate it in `from_interaction_config` (from the request) and in `from_http_request` (pass it in as a new parameter — update the two call sites in `server.rs` to pass `config.query_matching`). In `diff`, replace the `diff_operations(...)` call with `diff_operations_with(&expected_op, &actual_op, self.query_matching)`, and short-circuit to `diff_exact` when the mode is `Exact`.

- [ ] **Step 6: Run the full suite**

Run: `cargo test`
Expected: PASS.

Two classes of breakage to expect and fix:

1. Struct-literal construction of `CanonicalGraphqlRequest` in `server.rs` (two sites, roughly lines 158 and 292) — add `query_matching: config.query_matching,`.
2. Struct-literal construction of `GraphqlPluginRequest` in `pact-graphql-plugin/tests/plugin_flow.rs` and `pact-graphql-plugin/tests/interaction_tests.rs` — these now miss a field. `GraphqlPluginRequest` has a `Default` impl, so add `..GraphqlPluginRequest::default()` as the final entry in each literal rather than spelling out the new field. That also keeps them compiling when Task 8 adds `response_body_json`.

- [ ] **Step 7: Commit**

```bash
git add pact-graphql-plugin/src/ pact-graphql-plugin/tests/
git commit -m "feat: add exact/semantic/subset query matching modes"
```

---

### Task 6: Validate GraphQL responses against the schema and selection set

This is the objective-4 unlock. Given the schema, the query, and a response body, report every field the response contains that the schema does not define or the query did not request, plus type violations.

**Files:**
- Create: `pact-graphql-plugin/src/response.rs`
- Modify: `pact-graphql-plugin/src/lib.rs`
- Test: `pact-graphql-plugin/tests/response_tests.rs` (create)

**Interfaces:**
- Consumes: `SchemaIndex`, `TypeRef`, `OperationKind` (Task 1/2); `parse_and_inline`, `selection_set_of` (Task 3); `collect_fields` — make it `pub(crate)` in `query_ast.rs`.
- Produces: `pub(crate) struct ResponseMismatch { pub(crate) path: String, pub(crate) expected: String, pub(crate) actual: String, pub(crate) description: String }` and `pub(crate) fn validate_response(schema: &SchemaIndex, query_document: &str, operation_name: Option<&str>, response: &serde_json::Value) -> anyhow::Result<Vec<ResponseMismatch>>`.

- [ ] **Step 1: Write the failing test**

Create `pact-graphql-plugin/tests/response_tests.rs`:

```rust
use pact_graphql_plugin::test_support::validate_response_json;
use serde_json::json;

const SDL: &str = r#"
type Query { product(id: ID!): Product, products: [Product!]! }
type Product {
  id: ID!
  name: String
  status: ProductStatus!
  rating: Int
  category: Category
}
type Category { id: ID!, name: String! }
enum ProductStatus { ACTIVE ARCHIVED }
"#;

const QUERY: &str = "query GetProduct { product(id: \"10\") { id name status } }";

#[test]
fn accepts_a_well_formed_response() {
    let mismatches = validate_response_json(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({"data": {"product": {"id": "10", "name": "Backpack", "status": "ACTIVE"}}}),
    )
    .unwrap();
    assert!(mismatches.is_empty(), "got {mismatches:#?}");
}

#[test]
fn rejects_a_field_absent_from_the_schema() {
    let mismatches = validate_response_json(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({"data": {"product": {
            "id": "10", "name": "Backpack", "status": "ACTIVE", "internalSku": "INT-001"
        }}}),
    )
    .unwrap();
    assert_eq!(mismatches.len(), 1, "got {mismatches:#?}");
    assert_eq!(mismatches[0].path, "$.data.product.internalSku");
    assert!(
        mismatches[0].description.contains("not a field of type \"Product\""),
        "{}",
        mismatches[0].description
    );
}

#[test]
fn rejects_a_schema_field_the_query_did_not_request() {
    let mismatches = validate_response_json(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({"data": {"product": {
            "id": "10", "name": "Backpack", "status": "ACTIVE", "rating": 5
        }}}),
    )
    .unwrap();
    assert_eq!(mismatches.len(), 1, "got {mismatches:#?}");
    assert_eq!(mismatches[0].path, "$.data.product.rating");
    assert!(
        mismatches[0].description.contains("was not requested"),
        "{}",
        mismatches[0].description
    );
}

#[test]
fn rejects_a_missing_requested_field() {
    let mismatches = validate_response_json(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({"data": {"product": {"id": "10", "name": "Backpack"}}}),
    )
    .unwrap();
    assert_eq!(mismatches.len(), 1, "got {mismatches:#?}");
    assert_eq!(mismatches[0].path, "$.data.product.status");
    assert!(mismatches[0].description.contains("missing"), "{}", mismatches[0].description);
}

#[test]
fn rejects_an_invalid_enum_member() {
    let mismatches = validate_response_json(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({"data": {"product": {"id": "10", "name": "Backpack", "status": "SOLD_OUT"}}}),
    )
    .unwrap();
    assert_eq!(mismatches.len(), 1, "got {mismatches:#?}");
    assert_eq!(mismatches[0].path, "$.data.product.status");
    assert!(mismatches[0].expected.contains("ACTIVE"));
    assert_eq!(mismatches[0].actual, "SOLD_OUT");
}

#[test]
fn rejects_null_in_a_non_null_field() {
    let mismatches = validate_response_json(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({"data": {"product": {"id": "10", "name": "Backpack", "status": null}}}),
    )
    .unwrap();
    assert_eq!(mismatches.len(), 1, "got {mismatches:#?}");
    assert_eq!(mismatches[0].path, "$.data.product.status");
    assert!(mismatches[0].description.contains("non-null"), "{}", mismatches[0].description);
}

#[test]
fn accepts_null_in_a_nullable_field() {
    let mismatches = validate_response_json(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({"data": {"product": {"id": "10", "name": null, "status": "ACTIVE"}}}),
    )
    .unwrap();
    assert!(mismatches.is_empty(), "got {mismatches:#?}");
}

#[test]
fn rejects_a_scalar_of_the_wrong_json_type() {
    let query = "query Q { product(id: \"10\") { id rating } }";
    let mismatches = validate_response_json(
        SDL,
        query,
        Some("Q"),
        &json!({"data": {"product": {"id": "10", "rating": "five"}}}),
    )
    .unwrap();
    assert_eq!(mismatches.len(), 1, "got {mismatches:#?}");
    assert_eq!(mismatches[0].path, "$.data.product.rating");
    assert!(mismatches[0].description.contains("Int"), "{}", mismatches[0].description);
}

#[test]
fn walks_into_lists() {
    let query = "query Q { products { id category { name } } }";
    let mismatches = validate_response_json(
        SDL,
        query,
        Some("Q"),
        &json!({"data": {"products": [
            {"id": "1", "category": {"name": "Bags"}},
            {"id": "2", "category": {"name": "Bags", "slug": "bags"}}
        ]}}),
    )
    .unwrap();
    assert_eq!(mismatches.len(), 1, "got {mismatches:#?}");
    assert_eq!(mismatches[0].path, "$.data.products[1].category.slug");
}

#[test]
fn rejects_an_object_where_a_list_is_expected() {
    let query = "query Q { products { id } }";
    let mismatches = validate_response_json(
        SDL,
        query,
        Some("Q"),
        &json!({"data": {"products": {"id": "1"}}}),
    )
    .unwrap();
    assert_eq!(mismatches.len(), 1, "got {mismatches:#?}");
    assert_eq!(mismatches[0].path, "$.data.products");
    assert!(mismatches[0].description.contains("list"), "{}", mismatches[0].description);
}

#[test]
fn accepts_an_errors_envelope_with_null_data() {
    let mismatches = validate_response_json(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({
            "data": {"product": null},
            "errors": [{"message": "Product not found", "path": ["product"]}]
        }),
    )
    .unwrap();
    assert!(mismatches.is_empty(), "got {mismatches:#?}");
}

#[test]
fn rejects_an_error_without_a_message() {
    let mismatches = validate_response_json(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({"data": null, "errors": [{"code": "NOT_FOUND"}]}),
    )
    .unwrap();
    assert_eq!(mismatches.len(), 1, "got {mismatches:#?}");
    assert_eq!(mismatches[0].path, "$.errors[0].message");
}

#[test]
fn rejects_an_envelope_with_neither_data_nor_errors() {
    let mismatches =
        validate_response_json(SDL, QUERY, Some("GetProduct"), &json!({"meta": {}})).unwrap();
    assert!(
        mismatches.iter().any(|m| m.path == "$"),
        "got {mismatches:#?}"
    );
}

#[test]
fn rejects_an_unknown_top_level_key() {
    let mismatches = validate_response_json(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({
            "data": {"product": {"id": "10", "name": "Backpack", "status": "ACTIVE"}},
            "meta": {"traceId": "abc"}
        }),
    )
    .unwrap();
    assert_eq!(mismatches.len(), 1, "got {mismatches:#?}");
    assert_eq!(mismatches[0].path, "$.meta");
}
```

Add to `test_support` in `pact-graphql-plugin/src/lib.rs`:

```rust
    pub use crate::response::ResponseMismatch;

    pub fn validate_response_json(
        sdl: &str,
        query: &str,
        operation_name: Option<&str>,
        response: &serde_json::Value,
    ) -> anyhow::Result<Vec<ResponseMismatch>> {
        let index = SchemaIndex::from_sdl(sdl)?;
        crate::response::validate_response(&index, query, operation_name, response)
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test response_tests`
Expected: FAIL to compile with `could not find 'response' in the crate root`.

- [ ] **Step 3: Write the implementation**

Create `pact-graphql-plugin/src/response.rs`:

```rust
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
    let selected_keys: HashSet<&str> =
        selected.iter().map(|(key, _)| key.as_str()).collect();

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
```

In `pact-graphql-plugin/src/query_ast.rs`, change `fn collect_fields` and `fn selection_set_of` to `pub(crate) fn`.

Declare the module in `pact-graphql-plugin/src/lib.rs`:

```rust
pub(crate) mod response;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --test response_tests`
Expected: PASS — all 14 tests.

If `rejects_a_scalar_of_the_wrong_json_type` fails, check that `Int` uses `as_i64()` and not `is_number()` — a JSON string must not satisfy `Int`.

If `walks_into_lists` reports two mismatches instead of one, `validate_object` is probably reporting `slug` twice; confirm the "unexpected key" loop and the "missing field" loop do not overlap.

- [ ] **Step 5: Run the full suite**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add pact-graphql-plugin/src/response.rs pact-graphql-plugin/src/query_ast.rs pact-graphql-plugin/src/lib.rs pact-graphql-plugin/tests/response_tests.rs
git commit -m "feat: validate GraphQL responses against schema and selection set"
```

---

### Task 7: Derive matching rules from the schema

A user who writes `status: "ACTIVE"` should get an enum regex rule for free, and `name: "Backpack"` should become a type matcher rather than a literal. This is what makes the plugin *less* typing than raw Pact.

**Files:**
- Modify: `pact-graphql-plugin/src/response.rs`
- Test: `pact-graphql-plugin/tests/response_tests.rs` (append)

**Interfaces:**
- Consumes: everything from Task 6.
- Produces: `pub(crate) fn derive_matching_rules(schema: &SchemaIndex, query_document: &str, operation_name: Option<&str>, response: &Value) -> anyhow::Result<BTreeMap<String, DerivedRule>>` and `pub(crate) enum DerivedRule { Type, Regex(String) }`. Keys are Pact path expressions rooted at `$` with `[*]` for list elements.

- [ ] **Step 1: Write the failing test**

Append to `pact-graphql-plugin/tests/response_tests.rs`:

```rust
use pact_graphql_plugin::test_support::{derive_rules, DerivedRule};

#[test]
fn derives_a_type_matcher_for_scalars() {
    let rules = derive_rules(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({"data": {"product": {"id": "10", "name": "Backpack", "status": "ACTIVE"}}}),
    )
    .unwrap();
    assert_eq!(rules.get("$.data.product.id"), Some(&DerivedRule::Type));
    assert_eq!(rules.get("$.data.product.name"), Some(&DerivedRule::Type));
}

#[test]
fn derives_an_enum_regex() {
    let rules = derive_rules(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({"data": {"product": {"id": "10", "name": "Backpack", "status": "ACTIVE"}}}),
    )
    .unwrap();
    // Members are sorted so the regex is deterministic.
    assert_eq!(
        rules.get("$.data.product.status"),
        Some(&DerivedRule::Regex("^(ACTIVE|ARCHIVED)$".to_string()))
    );
}

#[test]
fn uses_a_wildcard_index_for_lists() {
    let query = "query Q { products { id category { name } } }";
    let rules = derive_rules(
        SDL,
        query,
        Some("Q"),
        &json!({"data": {"products": [{"id": "1", "category": {"name": "Bags"}}]}}),
    )
    .unwrap();
    assert_eq!(rules.get("$.data.products[*].id"), Some(&DerivedRule::Type));
    assert_eq!(
        rules.get("$.data.products[*].category.name"),
        Some(&DerivedRule::Type)
    );
    assert!(
        !rules.keys().any(|key| key.contains("[0]")),
        "list indices are collapsed to [*]: {rules:#?}"
    );
}

#[test]
fn does_not_emit_rules_for_composites_or_nulls() {
    let rules = derive_rules(
        SDL,
        QUERY,
        Some("GetProduct"),
        &json!({"data": {"product": {"id": "10", "name": null, "status": "ACTIVE"}}}),
    )
    .unwrap();
    assert!(!rules.contains_key("$.data.product"), "{rules:#?}");
    assert!(!rules.contains_key("$.data.product.name"), "{rules:#?}");
}
```

Add to `test_support` in `pact-graphql-plugin/src/lib.rs`:

```rust
    pub use crate::response::DerivedRule;

    pub fn derive_rules(
        sdl: &str,
        query: &str,
        operation_name: Option<&str>,
        response: &serde_json::Value,
    ) -> anyhow::Result<std::collections::BTreeMap<String, DerivedRule>> {
        let index = SchemaIndex::from_sdl(sdl)?;
        crate::response::derive_matching_rules(&index, query, operation_name, response)
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test response_tests`
Expected: FAIL to compile with `could not find 'DerivedRule'`.

- [ ] **Step 3: Write the implementation**

Append to `pact-graphql-plugin/src/response.rs`:

```rust
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DerivedRule {
    /// Pact `type` matcher — same JSON type, any value.
    Type,
    /// Pact `regex` matcher with the given pattern.
    Regex(String),
}

pub(crate) fn derive_matching_rules(
    schema: &SchemaIndex,
    query_document: &str,
    operation_name: Option<&str>,
    response: &Value,
) -> anyhow::Result<BTreeMap<String, DerivedRule>> {
    let operation = query_ast::parse_and_inline(query_document, operation_name)?;
    let root = schema
        .root_type(operation_kind_of(&operation))
        .ok_or_else(|| anyhow!("schema has no root type for this operation"))?
        .to_string();

    let mut rules = BTreeMap::new();

    if let Some(data) = response.get("data") {
        if !data.is_null() {
            derive_object_rules(
                schema,
                selection_set_of(&operation),
                &root,
                data,
                "$.data",
                &mut rules,
            );
        }
    }

    Ok(rules)
}

fn derive_object_rules(
    schema: &SchemaIndex,
    selection_set: &SelectionSet<'static, String>,
    parent_type: &str,
    value: &Value,
    path: &str,
    rules: &mut BTreeMap<String, DerivedRule>,
) {
    let Value::Object(object) = value else {
        return;
    };

    for (key, field) in collect_fields(selection_set) {
        if field.name == "__typename" {
            continue;
        }
        let Some(return_type) = schema.field_return_type(parent_type, &field.name) else {
            continue;
        };
        let Some(field_value) = object.get(&key) else {
            continue;
        };
        derive_value_rules(
            schema,
            return_type,
            &field.selection_set,
            field_value,
            &format!("{path}.{key}"),
            rules,
        );
    }
}

fn derive_value_rules(
    schema: &SchemaIndex,
    type_ref: &TypeRef,
    selection_set: &SelectionSet<'static, String>,
    value: &Value,
    path: &str,
    rules: &mut BTreeMap<String, DerivedRule>,
) {
    // A null tells us nothing about the shape; leave it as a literal.
    if value.is_null() {
        return;
    }

    if let Some(item_type) = type_ref.as_list_item() {
        let Value::Array(items) = value else {
            return;
        };
        // Every element shares one rule set, so walk only the first and key it
        // with a wildcard index.
        if let Some(first) = items.first() {
            derive_value_rules(
                schema,
                item_type,
                selection_set,
                first,
                &format!("{path}[*]"),
                rules,
            );
        }
        return;
    }

    let Some(named) = type_ref.unwrap_non_null().innermost_named() else {
        return;
    };

    if schema.is_composite_type(named) {
        derive_object_rules(schema, selection_set, named, value, path, rules);
        return;
    }

    if let Some(members) = schema.enum_values(named) {
        let mut allowed: Vec<&str> = members.iter().map(String::as_str).collect();
        allowed.sort_unstable();
        rules.insert(
            path.to_string(),
            DerivedRule::Regex(format!("^({})$", allowed.join("|"))),
        );
        return;
    }

    rules.insert(path.to_string(), DerivedRule::Type);
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --test response_tests`
Expected: PASS — all 18 tests.

- [ ] **Step 5: Commit**

```bash
git add pact-graphql-plugin/src/response.rs pact-graphql-plugin/src/lib.rs pact-graphql-plugin/tests/response_tests.rs
git commit -m "feat: derive matching rules from the GraphQL schema"
```

---

### Task 8: Wire response matching into the plugin protocol

Everything so far is library code with no route in. This task gives it one: a second content type so `compare_contents` can tell a response body from a request body, and a second `InteractionResponse` so `configure_interaction` can hand Pact core the response contents plus derived rules.

**Files:**
- Modify: `pact-graphql-plugin/src/server.rs:40` (constants), `:102-127` (`init_plugin`), `:58-82` (`configure`), `:137-211` (`compare_contents`)
- Modify: `pact-graphql-plugin/src/interaction.rs` (`GraphqlPluginRequest`, `GraphqlPluginConfig`)
- Test: `pact-graphql-plugin/tests/plugin_flow.rs` (append)

**Interfaces:**
- Consumes: `validate_response`, `derive_matching_rules`, `DerivedRule`, `ResponseMismatch` (Tasks 6-7); `SchemaIndex::from_sdl` (Task 1).
- Produces: catalogue entry `graphql-response`; content type constant `GRAPHQL_RESPONSE_CONTENT_TYPE = "application/graphql-response"`; `GraphqlPluginRequest.response_body_json: Option<String>` and the same on `GraphqlPluginConfig`.

- [ ] **Step 1: Write the failing test**

Append to `pact-graphql-plugin/tests/plugin_flow.rs`. The file already has `temp_plugin() -> (TempDir, GraphqlPlugin)` and `make_contents_config(&GraphqlPluginRequest) -> ConfigureInteractionRequest` — reuse both. Because `make_contents_config` serialises the whole `GraphqlPluginRequest`, the new `response_body_json` field flows through with no new helper.

First extend the existing import block at the top of the file:

```rust
use pact_plugin_driver::proto::{
    Body, CompareContentsRequest, ConfigureInteractionRequest, GenerateContentRequest,
    InitPluginRequest, PluginConfiguration,
};
```

(keep the other existing `use` lines as they are; `HashMap`, `to_proto_struct`, `TempDir`, `Request`, `PactPlugin`, `GraphqlPluginRequest`, `SchemaRegistry`, `GraphqlPlugin` are all still needed)

Then append:

```rust
const PRODUCT_SDL: &str = r#"
type Query { product(id: ID!): Product, products: [Product!]! }
type Product {
  id: ID!
  name: String
  status: ProductStatus!
  rating: Int
  category: Category
}
type Category { id: ID!, name: String! }
enum ProductStatus { ACTIVE ARCHIVED }
"#;

const PRODUCT_QUERY: &str = "query GetProduct { product(id: \"10\") { id name status } }";

fn product_request(response_body_json: Option<&str>) -> GraphqlPluginRequest {
    GraphqlPluginRequest {
        query_document: PRODUCT_QUERY.into(),
        operation_name: Some("GetProduct".into()),
        variables_json: None,
        transport: pact_graphql_plugin::encoder::Transport::JsonBody,
        schema_sdl: Some(PRODUCT_SDL.into()),
        query_matching: Default::default(),
        response_body_json: response_body_json.map(str::to_string),
    }
}

#[tokio::test]
async fn advertises_a_response_content_matcher() {
    let (_dir, plugin) = temp_plugin();
    let catalogue = plugin
        .init_plugin(Request::new(InitPluginRequest::default()))
        .await
        .expect("init succeeds")
        .into_inner()
        .catalogue;

    assert!(
        catalogue.iter().any(|entry| entry.key == "graphql-response"
            && entry.values.get("content-types")
                == Some(&"application/graphql-response".to_string())),
        "catalogue is {catalogue:#?}"
    );
}

#[tokio::test]
async fn configure_returns_a_response_part_with_derived_rules() {
    let (_dir, plugin) = temp_plugin();
    let request = product_request(Some(
        r#"{"data":{"product":{"id":"10","name":"Backpack","status":"ACTIVE"}}}"#,
    ));

    let response = plugin
        .configure_interaction(Request::new(make_contents_config(&request)))
        .await
        .expect("configure succeeds")
        .into_inner();

    assert_eq!(response.error, "");
    assert_eq!(response.interaction.len(), 2, "request and response parts");

    let response_part = response
        .interaction
        .iter()
        .find(|part| part.part_name == "response")
        .expect("a response part is returned");

    assert_eq!(
        response_part
            .contents
            .as_ref()
            .map(|body| body.content_type.as_str()),
        Some("application/json")
    );

    let status_rule = response_part
        .rules
        .get("$.data.product.status")
        .expect("enum rule is derived");
    assert_eq!(status_rule.rule[0].r#type, "regex");

    let name_rule = response_part
        .rules
        .get("$.data.product.name")
        .expect("scalar rule is derived");
    assert_eq!(name_rule.rule[0].r#type, "type");
}

#[tokio::test]
async fn configure_rejects_a_response_that_violates_the_schema() {
    let (_dir, plugin) = temp_plugin();
    let request = product_request(Some(
        r#"{"data":{"product":{"id":"10","name":"Backpack","status":"ACTIVE","internalSku":"X"}}}"#,
    ));

    let error = plugin
        .configure_interaction(Request::new(make_contents_config(&request)))
        .await
        .expect_err("configure fails")
        .message()
        .to_string();

    assert!(
        error.contains("internalSku") && error.contains("Product"),
        "error names the offending field: {error}"
    );
}

#[tokio::test]
async fn compare_contents_reports_response_mismatches() {
    let (_dir, plugin) = temp_plugin();

    // Configure first so we have the plugin configuration to compare against.
    let request = product_request(Some(
        r#"{"data":{"product":{"id":"10","name":"Backpack","status":"ACTIVE"}}}"#,
    ));
    let configured = plugin
        .configure_interaction(Request::new(make_contents_config(&request)))
        .await
        .expect("configure succeeds")
        .into_inner();
    let plugin_configuration: PluginConfiguration = configured
        .plugin_configuration
        .expect("plugin configuration");

    let actual = r#"{"data":{"product":{"id":"10","name":"Backpack","status":"SOLD_OUT"}}}"#;
    let compared = plugin
        .compare_contents(Request::new(CompareContentsRequest {
            expected: None,
            actual: Some(Body {
                content_type: "application/graphql-response".to_string(),
                content: Some(actual.as_bytes().to_vec()),
                content_type_hint: 0,
            }),
            allow_unexpected_keys: false,
            rules: Default::default(),
            plugin_configuration: Some(plugin_configuration),
        }))
        .await
        .expect("compare succeeds")
        .into_inner();

    let mismatches = compared
        .results
        .get("$.data.product.status")
        .expect("a mismatch is reported for status");
    assert_eq!(mismatches.mismatches.len(), 1, "got {mismatches:#?}");
}
```

Note: `compare_contents` returns `CompareContentsResponse`, whose `results` is a `HashMap<String, ContentMismatches>` keyed by path — check `build_compare_contents_response` in `server.rs` to confirm the exact keying it uses and adjust the final assertion to match rather than guessing.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test plugin_flow`
Expected: FAIL — `advertises_a_response_content_matcher` fails its assertion, and the other three fail to compile or panic on the missing `response_body_json` field.

- [ ] **Step 3: Add the config field**

In `pact-graphql-plugin/src/interaction.rs`, add to `GraphqlPluginRequest`:

```rust
    pub response_body_json: Option<String>,
```

with `response_body_json: None` in its `Default` impl. Add the same `Option<String>` field to `GraphqlPluginConfig` and `GraphqlPluginConfigWire`, wiring it through the `Serialize`/`Deserialize` impls exactly as `variables_json` is wired. Carry it through `GraphqlInteractionBuilder::build` — capture it before the `from_interaction_config` call, as you did for `query_matching` in Task 5.

- [ ] **Step 4: Register the response content matcher**

In `pact-graphql-plugin/src/server.rs`, add near the existing `GRAPHQL_JSON_CONTENT_TYPE` constant:

```rust
const GRAPHQL_RESPONSE_CONTENT_TYPE: &str = "application/graphql-response";
```

In `init_plugin`, append a third catalogue entry:

```rust
CatalogueEntry {
    r#type: EntryType::ContentMatcher as i32,
    key: "graphql-response".to_string(),
    values: HashMap::from([(
        "content-types".to_string(),
        GRAPHQL_RESPONSE_CONTENT_TYPE.to_string(),
    )]),
},
```

- [ ] **Step 5: Return a response part from `configure`**

`server.rs` currently imports `anyhow::{anyhow, Context, Result}` — it does **not** import `bail`, which the code below uses. Change that line to:

```rust
use anyhow::{anyhow, bail, Context, Result};
```

Also add the schema-index import alongside the other `use crate::` lines:

```rust
use crate::schema_index::SchemaIndex;
```

In `GraphqlPlugin::configure`, after building `body` and `plugin_cfg`, add:

```rust
let mut interactions = vec![InteractionResponse {
    contents: Some(body),
    plugin_configuration: Some(plugin_cfg.clone()),
    part_name: "request".to_string(),
    ..InteractionResponse::default()
}];

if let Some(response_json) = config.response_body_json.as_deref() {
    let response_value: serde_json::Value = serde_json::from_str(response_json)
        .context("response_body_json must be valid JSON")?;

    let sdl = config
        .inline_schema
        .as_ref()
        .map(|_| CanonicalGraphqlRequest {
            payload: config.request.clone(),
            inline_schema: config.inline_schema.clone(),
            query_matching: config.query_matching,
        })
        .map(|canonical| canonical.inline_schema_sdl())
        .transpose()?
        .flatten();

    if let Some(sdl) = sdl {
        let schema_index = SchemaIndex::from_sdl(&sdl)
            .context("failed to parse GraphQL schema SDL")?;

        let mismatches = crate::response::validate_response(
            &schema_index,
            &config.query_document,
            config.operation_name.as_deref(),
            &response_value,
        )?;

        if !mismatches.is_empty() {
            let detail = mismatches
                .iter()
                .map(|mismatch| format!("{}: {}", mismatch.path, mismatch.description))
                .collect::<Vec<_>>()
                .join("; ");
            bail!("GraphQL response validation failed: {detail}");
        }

        let derived = crate::response::derive_matching_rules(
            &schema_index,
            &config.query_document,
            config.operation_name.as_deref(),
            &response_value,
        )?;

        interactions.push(InteractionResponse {
            contents: Some(Body {
                content_type: "application/json".to_string(),
                content: Some(response_json.as_bytes().to_vec()),
                content_type_hint: proto::body::ContentTypeHint::Text as i32,
            }),
            rules: derived_rules_to_proto(derived),
            plugin_configuration: Some(plugin_cfg.clone()),
            part_name: "response".to_string(),
            ..InteractionResponse::default()
        });
    }
}
```

Add the conversion helper near `build_compare_contents_response`:

```rust
fn derived_rules_to_proto(
    derived: std::collections::BTreeMap<String, crate::response::DerivedRule>,
) -> HashMap<String, proto::MatchingRules> {
    derived
        .into_iter()
        .map(|(path, rule)| {
            let proto_rule = match rule {
                crate::response::DerivedRule::Type => proto::MatchingRule {
                    r#type: "type".to_string(),
                    values: None,
                },
                crate::response::DerivedRule::Regex(pattern) => proto::MatchingRule {
                    r#type: "regex".to_string(),
                    values: Some(Struct {
                        fields: std::collections::BTreeMap::from([(
                            "regex".to_string(),
                            prost_types::Value {
                                kind: Some(prost_types::value::Kind::StringValue(pattern)),
                            },
                        )]),
                    }),
                },
            };
            (
                path,
                proto::MatchingRules {
                    rule: vec![proto_rule],
                },
            )
        })
        .collect()
}
```

Then return `interaction: interactions` from `configure` instead of the single-element `vec![...]`.

Note the existing `InteractionResponse` for the request part did not set `part_name`; setting it to `"request"` is required now that there are two parts.

- [ ] **Step 6: Branch on the content type in `compare_contents`**

At the top of `compare_contents`, after decoding `config` and before building `expected`, insert:

```rust
let actual_body = actual
    .ok_or_else(|| Status::invalid_argument("actual request body is required"))?;
let actual_content_type = actual_body.content_type.clone();

if actual_content_type.starts_with(GRAPHQL_RESPONSE_CONTENT_TYPE) {
    let bytes = actual_body
        .content
        .ok_or_else(|| Status::invalid_argument("actual body content is required"))?;
    let response_value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|err| Status::invalid_argument(err.to_string()))?;

    let canonical = CanonicalGraphqlRequest {
        payload: config.request.clone(),
        inline_schema: config.inline_schema.clone(),
        query_matching: config.query_matching,
    };
    let Some(sdl) = canonical.inline_schema_sdl().map_err(to_status)? else {
        // No schema was supplied, so there is nothing to validate against.
        return Ok(Response::new(build_compare_contents_response(Vec::new())));
    };
    let schema_index = SchemaIndex::from_sdl(&sdl).map_err(to_status)?;

    let mismatches = crate::response::validate_response(
        &schema_index,
        &config.query_document,
        config.operation_name.as_deref(),
        &response_value,
    )
    .map_err(to_status)?;

    let as_request: Vec<RequestMismatch> = mismatches
        .into_iter()
        .map(|mismatch| {
            RequestMismatch::new(
                &mismatch.path,
                mismatch.expected,
                mismatch.actual,
                &mismatch.description,
            )
        })
        .collect();

    return Ok(Response::new(build_compare_contents_response(as_request)));
}
```

`RequestMismatch::new` is currently private to `graphql_payload.rs`; make it `pub(crate)`.

The rest of `compare_contents` continues to use `actual_body` — replace its existing `let actual_body = actual.ok_or_else(...)` line, which is now redundant.

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test --test plugin_flow`
Expected: PASS — all four new tests plus the file's existing tests.

Run: `cargo test`
Expected: PASS — the whole suite.

- [ ] **Step 8: Verify end to end against the example**

Run:

```bash
just install
cd examples/js/product-consumer && npm install && npm run test
```

Expected: the suite runs. The `runtime mismatch` test now fails with the field-path message from Task 4 — if you did not already update its assertion in Task 4 Step 6, update it now.

The `response not in schema` test at `examples/js/product-consumer/pact.test.ts:449` still passes only because it validates locally. Leave it — rewiring the JS DSL to send `response_body_json` is Plan 2. Add a comment above it:

```ts
// TODO(plan-2): once the JS DSL forwards response_body_json to the plugin, this
// assertion moves to the plugin and this local check can be deleted.
```

- [ ] **Step 9: Commit**

```bash
git add pact-graphql-plugin/src/ pact-graphql-plugin/tests/ examples/js/product-consumer/pact.test.ts
git commit -m "feat: wire GraphQL response matching into the plugin protocol"
```

---

## Definition of Done

- `cargo test` passes from the repo root with no warnings.
- `just install && cd examples/js/product-consumer && npm test` runs, with the runtime-mismatch assertion updated to the new field-path message.
- A query that differs only in formatting, comments, or fragment structure produces zero mismatches.
- A query that differs in one nested field produces exactly one mismatch, whose path names that field.
- A response containing a field absent from the schema, absent from the selection set, of the wrong scalar type, or holding an invalid enum member is rejected at `configure_interaction` with a message naming the field.
- Enum fields in the response carry a derived `regex` rule; other scalars carry a derived `type` rule.

## Out of Scope (follow-up plans)

- **Plan 2 — bindings:** strip `normalizeQuery` / `serializeVariables` / `graphqlRequestBody` from `js/pact-graphql-helper` and `go/pactgraphql`; fluent builder returning a bound client; forward `response_body_json` and `query_matching` from the DSL; matchers inside `variables`; shared conformance fixtures.
- **Plan 3 — subscriptions and ergonomics:** `graphql-transport-ws` framing owned by the plugin, per-message descriptions, plugin version resolution without an environment variable, configurable path/method, GET/`query_string` transport in the DSL.
