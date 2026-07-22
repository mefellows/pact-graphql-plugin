use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use anyhow::{bail, Context};
use graphql_parser::schema::{
    parse_schema, Definition as SchemaDefinition, Document as SchemaDocument, Field as SchemaField,
    InputValue as SchemaInputValue, Type as SchemaType, TypeDefinition, TypeExtension,
};

pub(crate) const INTROSPECTION_SDL: &str = r#"
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

scalar ID
scalar String
scalar Int
scalar Float
scalar Boolean
"#;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum OperationKind {
    Query,
    Mutation,
    Subscription,
}

#[derive(Clone, Debug)]
pub struct SchemaIndex {
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
                    .or_insert_with(|| TypeInfo::Enum(EnumTypeInfo::default()));
                match entry {
                    TypeInfo::Enum(existing) => {
                        existing
                            .values
                            .extend(enum_ext.values.into_iter().map(|v| v.name));
                    }
                    other => {
                        bail!(
                            "type `{}` cannot be extended as enum because it was previously defined as {}",
                            enum_ext.name,
                            other.kind()
                        );
                    }
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

    pub(crate) fn type_info(&self, type_name: &str) -> Option<&TypeInfo> {
        self.types.get(type_name)
    }

    pub(crate) fn is_composite_type(&self, type_name: &str) -> bool {
        matches!(
            self.types.get(type_name),
            Some(TypeInfo::Object(_) | TypeInfo::Interface(_) | TypeInfo::Union(_))
        )
    }

    pub(crate) fn is_union_type(&self, type_name: &str) -> bool {
        matches!(self.types.get(type_name), Some(TypeInfo::Union(_)))
    }

    pub(crate) fn is_query_root(&self, type_name: &str) -> bool {
        self.query_root == type_name
    }

    pub(crate) fn ensure_type_exists(&self, type_name: &str) -> anyhow::Result<()> {
        if self.types.contains_key(type_name) {
            Ok(())
        } else {
            bail!("type `{}` not found in schema", type_name)
        }
    }

    pub(crate) fn field<'a>(
        &'a self,
        parent: &'a TypeInfo,
        field_name: &str,
    ) -> Option<&'a FieldInfo> {
        match parent {
            TypeInfo::Object(object) => object.fields.get(field_name),
            TypeInfo::Interface(interface) => interface.fields.get(field_name),
            _ => None,
        }
    }

    pub(crate) fn runtime_types(&self, type_name: &str) -> anyhow::Result<HashSet<String>> {
        match self.types.get(type_name) {
            Some(TypeInfo::Object(_)) => Ok(HashSet::from([type_name.to_string()])),
            Some(TypeInfo::Interface(_)) => self.interface_runtime_types(type_name),
            Some(TypeInfo::Union(union)) => Ok(union.members.clone()),
            Some(TypeInfo::Scalar) | Some(TypeInfo::Enum(_)) | Some(TypeInfo::InputObject) => {
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

    pub fn enum_values(&self, type_name: &str) -> Option<&HashSet<String>> {
        match self.types.get(type_name) {
            Some(TypeInfo::Enum(info)) => Some(&info.values),
            _ => None,
        }
    }

    pub fn is_enum(&self, type_name: &str) -> bool {
        matches!(self.types.get(type_name), Some(TypeInfo::Enum(_)))
    }

    pub fn is_scalar(&self, type_name: &str) -> bool {
        matches!(self.types.get(type_name), Some(TypeInfo::Scalar))
    }

    pub fn field_return_type(
        &self,
        parent_type: &str,
        field_name: &str,
    ) -> Option<&TypeRef> {
        let parent = self.types.get(parent_type)?;
        self.field(parent, field_name).map(|info| &info.return_type)
    }

    pub(crate) fn ensure_fragment_applicable(
        &self,
        parent_type: &str,
        condition: &str,
    ) -> anyhow::Result<()> {
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
pub(crate) enum TypeInfo {
    Object(ObjectTypeInfo),
    Interface(InterfaceTypeInfo),
    Union(UnionTypeInfo),
    Scalar,
    Enum(EnumTypeInfo),
    InputObject,
}

impl TypeInfo {
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            TypeInfo::Object(_) => "object",
            TypeInfo::Interface(_) => "interface",
            TypeInfo::Union(_) => "union",
            TypeInfo::Scalar => "scalar",
            TypeInfo::Enum(_) => "enum",
            TypeInfo::InputObject => "input object",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct EnumTypeInfo {
    pub(crate) values: HashSet<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct ObjectTypeInfo {
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
pub(crate) struct InterfaceTypeInfo {
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
pub(crate) struct UnionTypeInfo {
    members: HashSet<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct FieldCollection {
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

    pub(crate) fn get(&self, name: &str) -> Option<&FieldInfo> {
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
pub(crate) struct FieldInfo {
    pub(crate) return_type: TypeRef,
    pub(crate) arguments: HashMap<String, InputValueInfo>,
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

    pub(crate) fn composite_type<'a>(&'a self, schema_index: &'a SchemaIndex) -> Option<&'a str> {
        let named = self.return_type.innermost_named()?;
        if schema_index.is_composite_type(named) {
            Some(named)
        } else {
            None
        }
    }

    pub(crate) fn argument(&self, name: &str) -> Option<&InputValueInfo> {
        self.arguments.get(name)
    }
}

#[derive(Clone, Debug)]
pub enum TypeRef {
    Named(String),
    List(Box<TypeRef>),
    NonNull(Box<TypeRef>),
}

impl TypeRef {
    pub fn innermost_named(&self) -> Option<&str> {
        match self {
            TypeRef::Named(name) => Some(name.as_str()),
            TypeRef::List(inner) | TypeRef::NonNull(inner) => inner.innermost_named(),
        }
    }

    pub fn is_non_null(&self) -> bool {
        matches!(self, TypeRef::NonNull(_))
    }

    /// Strips a single `NonNull` wrapper, if present.
    pub fn unwrap_non_null(&self) -> &TypeRef {
        match self {
            TypeRef::NonNull(inner) => inner,
            other => other,
        }
    }

    /// Returns the item type when this is a list, ignoring any outer `NonNull`.
    pub fn as_list_item(&self) -> Option<&TypeRef> {
        match self.unwrap_non_null() {
            TypeRef::List(inner) => Some(inner),
            _ => None,
        }
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
pub(crate) struct InputValueInfo {
    type_ref: TypeRef,
    has_default: bool,
}

impl InputValueInfo {
    pub(crate) fn is_required(&self) -> bool {
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
