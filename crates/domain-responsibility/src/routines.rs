//! Saved, revisioned Routine definitions.
//!
//! A Routine is a passive work template. This boundary deliberately does not
//! schedule work, create Tasks, or execute triggers. Automation and Task adapters
//! may pin the immutable revision returned here.

use crate::{ContractObject, PlacementPreference, PrincipalKind, PrincipalRef};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RoutineStatus {
    Active,
    Archived,
}

/// Schema owned elsewhere remains an object until the owning contract validator is
/// integrated. Keeping it typed as JSON prevents this domain from inventing a second
/// schema for outputs, bindings, capabilities, budgets, or verification policy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutineRevisionInput {
    pub objective_template: String,
    pub instructions: String,
    pub input_schema: ContractObject,
    pub constraints: Vec<String>,
    pub non_goals: Vec<String>,
    pub required_outputs: Vec<ContractObject>,
    pub acceptance_criteria: Vec<ContractObject>,
    pub approvals_required: Vec<ContractObject>,
    pub input_bindings: Vec<ContractObject>,
    pub required_capabilities: Vec<ContractObject>,
    pub preferred_agent_binding_id: Option<String>,
    pub placement_preference: PlacementPreference,
    pub budget_ceiling: Option<ContractObject>,
    pub verification_policy: ContractObject,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoutineRevision {
    pub routine_id: String,
    pub revision: u64,
    #[serde(flatten)]
    pub definition: RoutineRevisionInput,
    pub authored_by: PrincipalRef,
    pub created_at: String,
}

/// Fully materialized, bounded inputs for one manual Routine run. Resource references
/// remain pinned separately in the resulting TaskSpec; the objective only includes a
/// safe label for those attachments.
#[derive(Clone, Debug, PartialEq)]
pub struct RoutineMaterialization {
    pub objective: String,
    pub constraints: Vec<String>,
    pub non_goals: Vec<String>,
    pub input_refs: Vec<serde_json::Value>,
    pub required_outputs: Vec<serde_json::Value>,
    pub acceptance_criteria: Vec<serde_json::Value>,
    pub approvals_required: Vec<serde_json::Value>,
    pub placement_preference: serde_json::Value,
    pub budget_ceiling: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Routine {
    pub routine_id: String,
    pub workspace_id: String,
    pub name: String,
    pub current_revision: u64,
    pub status: RoutineStatus,
    pub created_at: String,
    pub updated_at: String,
    pub version: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutineError {
    NotFound,
    AlreadyExists,
    Unauthorized,
    VersionConflict,
    VersionOverflow,
    RevisionOverflow,
    InvalidDefinition,
    Archived,
    ArchiveBlocked,
    IdempotencyConflict,
    WorkspaceArchived,
    Storage,
}
impl std::fmt::Display for RoutineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RoutineError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutineOwnerScope {
    /// Transport-authenticated principal; the store rechecks Workspace ownership.
    pub principal_id: String,
    pub workspace_id: String,
    pub request_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "command",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum RoutineCommand {
    Create {
        routine_id: String,
        name: String,
        revision: RoutineRevisionInput,
    },
    Revise {
        routine_id: String,
        expected_version: u64,
        revision: RoutineRevisionInput,
    },
    Archive {
        routine_id: String,
        expected_version: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoutineEvent {
    pub kind: String,
    pub payload: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoutineMutation {
    pub routine: Routine,
    pub expected_version: Option<u64>,
    pub append_revision: Option<RoutineRevision>,
    pub event: RoutineEvent,
}

pub trait RoutineTransaction {
    fn now(&self) -> String;
    fn routine(&mut self, id: &str) -> Result<Option<(Routine, RoutineRevision)>, RoutineError>;
    fn validate_references(
        &mut self,
        workspace_id: &str,
        revision: &RoutineRevisionInput,
    ) -> Result<(), RoutineError>;
    fn has_enabled_automation_references(&mut self, routine_id: &str)
    -> Result<bool, RoutineError>;
    fn commit(&mut self, mutation: RoutineMutation) -> Result<Routine, RoutineError>;
}

pub trait RoutineStore {
    /// Authorize before replay, bind RequestId to the full command fingerprint, and
    /// atomically persist the head, immutable revision, event, snapshot, and receipt.
    fn transaction<F>(
        &mut self,
        scope: &RoutineOwnerScope,
        fingerprint: &str,
        operation: F,
    ) -> Result<Routine, RoutineError>
    where
        F: Fn(&mut dyn RoutineTransaction) -> Result<Routine, RoutineError> + Send + 'static;
}

pub struct RoutineService<S> {
    store: S,
}
impl<S: RoutineStore> RoutineService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn into_store(self) -> S {
        self.store
    }

    pub fn execute(
        &mut self,
        scope: &RoutineOwnerScope,
        command: RoutineCommand,
    ) -> Result<Routine, RoutineError> {
        if [&scope.principal_id, &scope.workspace_id, &scope.request_id]
            .iter()
            .any(|value| value.trim().is_empty())
        {
            return Err(RoutineError::Unauthorized);
        }
        let fingerprint = command_fingerprint(&command)?;
        let owned_scope = scope.clone();
        self.store.transaction(scope, &fingerprint, move |tx| {
            decide(tx, &owned_scope, command.clone())
        })
    }
}

pub fn validate_routine_revision(input: &RoutineRevisionInput) -> Result<(), RoutineError> {
    fn valid_text(value: &str, max: usize) -> bool {
        !value.trim().is_empty()
            && value.chars().count() <= max
            && !value
                .chars()
                .any(|ch| ch.is_control() && ch != '\n' && ch != '\t')
    }
    if !valid_text(&input.objective_template, 4000)
        || !valid_text(&input.instructions, 32_000)
        || input.preferred_agent_binding_id.as_ref().is_some_and(|id| {
            id.trim().is_empty() || id.len() > 256 || id.chars().any(char::is_control)
        })
        || input.constraints.len() > 100
        || input.non_goals.len() > 100
        || input
            .constraints
            .iter()
            .chain(&input.non_goals)
            .any(|s| !valid_text(s, 2000))
    {
        return Err(RoutineError::InvalidDefinition);
    }
    validate_routine_input_contract(input)?;
    // Ensure values are finite and serializable without attempting to validate schemas
    // owned by other contracts here.
    canonical(&serde_json::to_value(input).map_err(|_| RoutineError::InvalidDefinition)?)?;
    Ok(())
}

/// The local V1 Routine runner implements this bounded JSON Schema subset. Rejecting
/// unsupported keywords at revision admission prevents a saved definition from
/// appearing runnable while silently ignoring part of its schema.
fn validate_routine_input_contract(input: &RoutineRevisionInput) -> Result<(), RoutineError> {
    use serde_json::{Map, Value};
    fn keys(map: &Map<String, Value>, allowed: &[&str]) -> bool {
        map.keys().all(|key| allowed.contains(&key.as_str()))
    }
    fn schema(value: &Value, depth: usize) -> Result<(), RoutineError> {
        if depth > 16 {
            return Err(RoutineError::InvalidDefinition);
        }
        let object = value.as_object().ok_or(RoutineError::InvalidDefinition)?;
        if !keys(
            object,
            &[
                "type",
                "properties",
                "required",
                "additionalProperties",
                "maxProperties",
                "minProperties",
                "maxLength",
                "minLength",
                "maxItems",
                "minItems",
                "items",
                "enum",
            ],
        ) {
            return Err(RoutineError::InvalidDefinition);
        }
        let kind = object
            .get("type")
            .and_then(Value::as_str)
            .ok_or(RoutineError::InvalidDefinition)?;
        if ![
            "object", "array", "string", "integer", "number", "boolean", "null",
        ]
        .contains(&kind)
        {
            return Err(RoutineError::InvalidDefinition);
        }
        for bound in [
            "maxProperties",
            "minProperties",
            "maxLength",
            "minLength",
            "maxItems",
            "minItems",
        ] {
            if object
                .get(bound)
                .is_some_and(|value| value.as_u64().is_none())
            {
                return Err(RoutineError::InvalidDefinition);
            }
        }
        if object.get("enum").is_some_and(|value| {
            !value.is_array()
                || value
                    .as_array()
                    .is_some_and(|items| items.is_empty() || items.len() > 128)
        }) {
            return Err(RoutineError::InvalidDefinition);
        }
        match kind {
            "object" => {
                let properties = object
                    .get("properties")
                    .and_then(Value::as_object)
                    .ok_or(RoutineError::InvalidDefinition)?;
                if properties.len() > 64
                    || object
                        .get("additionalProperties")
                        .is_some_and(|value| value != &Value::Bool(false))
                {
                    return Err(RoutineError::InvalidDefinition);
                }
                if object
                    .get("required")
                    .is_some_and(|value| !value.is_array())
                {
                    return Err(RoutineError::InvalidDefinition);
                }
                let required = object
                    .get("required")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let mut seen = std::collections::HashSet::new();
                for name in &required {
                    let name = name.as_str().ok_or(RoutineError::InvalidDefinition)?;
                    if !properties.contains_key(name) || !seen.insert(name) {
                        return Err(RoutineError::InvalidDefinition);
                    }
                }
                for nested in properties.values() {
                    schema(nested, depth + 1)?;
                }
            }
            "array" => schema(
                object.get("items").ok_or(RoutineError::InvalidDefinition)?,
                depth + 1,
            )?,
            _ if object.contains_key("properties")
                || object.contains_key("required")
                || object.contains_key("items")
                || object.contains_key("additionalProperties") =>
            {
                return Err(RoutineError::InvalidDefinition);
            }
            _ => {}
        }
        Ok(())
    }
    fn schema_at<'a>(root: &'a Value, parts: &[String]) -> Option<&'a Value> {
        let mut node = root;
        for part in parts {
            let object = node.as_object()?;
            node = match object.get("type")?.as_str()? {
                "object" => object.get("properties")?.get(part)?,
                "array" => {
                    part.parse::<usize>().ok()?;
                    object.get("items")?
                }
                _ => return None,
            };
        }
        Some(node)
    }
    fn pointer_parts(pointer: &str) -> Option<Vec<String>> {
        if !pointer.starts_with('/') || pointer.len() > 512 {
            return None;
        }
        pointer[1..]
            .split('/')
            .map(|part| {
                let mut output = String::new();
                let mut chars = part.chars();
                while let Some(ch) = chars.next() {
                    if ch == '~' {
                        match chars.next()? {
                            '0' => output.push('~'),
                            '1' => output.push('/'),
                            _ => return None,
                        }
                    } else {
                        output.push(ch);
                    }
                }
                Some(output)
            })
            .collect()
    }
    fn encode_pointer_part(part: &str) -> String {
        part.replace('~', "~0").replace('/', "~1")
    }
    fn template_names(template: &str) -> Result<std::collections::HashSet<String>, RoutineError> {
        let mut names = std::collections::HashSet::new();
        let mut rest = template;
        while let Some(start) = rest.find("{{") {
            let after = start + 2;
            let close = rest[after..]
                .find("}}")
                .ok_or(RoutineError::InvalidDefinition)?
                + after;
            let name = &rest[after..close];
            let valid = !name.is_empty()
                && name.len() <= 64
                && (name.starts_with('_') || name.as_bytes()[0].is_ascii_alphabetic())
                && name.bytes().enumerate().all(|(index, byte)| {
                    byte.is_ascii_alphanumeric() || byte == b'_' || (index > 0 && byte == b'-')
                });
            if !valid {
                return Err(RoutineError::InvalidDefinition);
            }
            names.insert(name.to_owned());
            rest = &rest[(close + 2)..];
        }
        if rest.contains("}}") {
            return Err(RoutineError::InvalidDefinition);
        }
        Ok(names)
    }

    let schema_value =
        serde_json::to_value(&input.input_schema).map_err(|_| RoutineError::InvalidDefinition)?;
    let root = if schema_value.as_object().is_some_and(Map::is_empty) {
        serde_json::json!({"type":"object","properties":{},"required":[],"additionalProperties":false})
    } else {
        schema_value
    };
    schema(&root, 0)?;
    if root.get("type").and_then(Value::as_str) != Some("object") {
        return Err(RoutineError::InvalidDefinition);
    }
    let properties = root
        .get("properties")
        .and_then(Value::as_object)
        .ok_or(RoutineError::InvalidDefinition)?;
    let root_object = root.as_object().ok_or(RoutineError::InvalidDefinition)?;
    if !keys(
        root_object,
        &[
            "type",
            "properties",
            "required",
            "additionalProperties",
            "maxProperties",
            "minProperties",
        ],
    ) || root
        .get("additionalProperties")
        .is_some_and(|value| value != &Value::Bool(false))
        || root
            .get("maxProperties")
            .and_then(Value::as_u64)
            .is_some_and(|value| value > 64)
        || root.get("required").is_some_and(|value| !value.is_array())
    {
        return Err(RoutineError::InvalidDefinition);
    }
    let schema_required_count = root
        .get("required")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    if root
        .get("minProperties")
        .and_then(Value::as_u64)
        .is_some_and(|min| min > properties.len() as u64)
        || root
            .get("maxProperties")
            .and_then(Value::as_u64)
            .is_some_and(|max| max < schema_required_count as u64)
    {
        return Err(RoutineError::InvalidDefinition);
    }
    let bindings = &input.input_bindings;
    if bindings.len() > 64 {
        return Err(RoutineError::InvalidDefinition);
    }
    let mut pointers = std::collections::HashSet::new();
    let mut variables = std::collections::HashSet::new();
    for binding in bindings {
        let pointer = binding
            .get("source_pointer")
            .and_then(Value::as_str)
            .ok_or(RoutineError::InvalidDefinition)?;
        let variable = binding
            .get("template_variable")
            .and_then(Value::as_str)
            .ok_or(RoutineError::InvalidDefinition)?;
        let kind = binding
            .get("value_kind")
            .and_then(Value::as_str)
            .ok_or(RoutineError::InvalidDefinition)?;
        if binding.keys().any(|key| {
            ![
                "source_pointer",
                "template_variable",
                "value_kind",
                "required",
                "max_bytes",
            ]
            .contains(&key.as_str())
        }) || !binding.get("required").is_some_and(Value::is_boolean)
            || !pointers.insert(pointer.to_owned())
            || !variables.insert(variable.to_owned())
            || !["TEXT", "RESOURCE_REF"].contains(&kind)
        {
            return Err(RoutineError::InvalidDefinition);
        }
        if let Some(limit) = binding.get("max_bytes") {
            if limit
                .as_u64()
                .is_none_or(|value| value == 0 || value > 16 * 1024)
            {
                return Err(RoutineError::InvalidDefinition);
            }
        }
        let parts = pointer_parts(pointer).ok_or(RoutineError::InvalidDefinition)?;
        if parts.len() != 1 {
            return Err(RoutineError::InvalidDefinition);
        }
        let field = schema_at(&root, &parts).ok_or(RoutineError::InvalidDefinition)?;
        let expected = if kind == "TEXT" { "string" } else { "object" };
        if field.get("type").and_then(Value::as_str) != Some(expected) {
            return Err(RoutineError::InvalidDefinition);
        }
        let field_object = field.as_object().ok_or(RoutineError::InvalidDefinition)?;
        if kind == "TEXT" {
            if !keys(field_object, &["type", "maxLength", "minLength", "enum"])
                || field_object
                    .get("maxLength")
                    .and_then(Value::as_u64)
                    .is_some_and(|value| value > 16 * 1024)
                || field_object
                    .get("minLength")
                    .and_then(Value::as_u64)
                    .is_some_and(|value| value > 16 * 1024)
                || field_object
                    .get("maxLength")
                    .and_then(Value::as_u64)
                    .zip(field_object.get("minLength").and_then(Value::as_u64))
                    .is_some_and(|(max, min)| min > max)
                || field_object
                    .get("enum")
                    .and_then(Value::as_array)
                    .is_some_and(|values| values.iter().any(|value| !value.is_string()))
            {
                return Err(RoutineError::InvalidDefinition);
            }
        }
        if kind == "RESOURCE_REF" {
            if !keys(
                field_object,
                &["type", "properties", "required", "additionalProperties"],
            ) {
                return Err(RoutineError::InvalidDefinition);
            }
            let object = field.as_object().ok_or(RoutineError::InvalidDefinition)?;
            let fields = object
                .get("properties")
                .and_then(Value::as_object)
                .ok_or(RoutineError::InvalidDefinition)?;
            if object
                .get("required")
                .is_some_and(|value| !value.is_array())
            {
                return Err(RoutineError::InvalidDefinition);
            }
            let required = object
                .get("required")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if object.get("additionalProperties") != Some(&Value::Bool(false))
                || fields.len() != 3
                || required.len() != 3
                || ["workspace_id", "resource_id", "revision_id"]
                    .iter()
                    .any(|name| {
                        !fields.get(*name).is_some_and(|schema| {
                            schema.get("type").and_then(Value::as_str) == Some("string")
                        }) || !required.iter().any(|value| value.as_str() == Some(name))
                            || fields
                                .get(*name)
                                .and_then(Value::as_object)
                                .is_none_or(|schema| !keys(schema, &["type"]))
                    })
            {
                return Err(RoutineError::InvalidDefinition);
            }
        }
        let schema_required = root
            .get("required")
            .and_then(Value::as_array)
            .is_some_and(|items| items.contains(&Value::String(parts[0].clone())));
        if schema_required
            != binding
                .get("required")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            return Err(RoutineError::InvalidDefinition);
        }
    }
    let placeholders = template_names(&input.objective_template)?;
    if placeholders != variables {
        return Err(RoutineError::InvalidDefinition);
    }
    let required = root
        .get("required")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for name in required.iter().filter_map(Value::as_str) {
        let escaped = encode_pointer_part(name);
        let bound = pointers.iter().any(|pointer| {
            pointer == &format!("/{escaped}") || pointer.starts_with(&format!("/{escaped}/"))
        });
        if !bound {
            return Err(RoutineError::InvalidDefinition);
        }
    }
    for property in properties.keys() {
        let escaped = encode_pointer_part(property);
        if !pointers.contains(&format!("/{escaped}")) {
            return Err(RoutineError::InvalidDefinition);
        }
    }
    Ok(())
}

/// Validates and renders the bounded JSON-Schema subset used by local manual Routine
/// runs. Unsupported schema keywords fail closed instead of being ignored. Schema and
/// input values are always taken from the same immutable RoutineRevision.
pub fn materialize_routine_inputs(
    revision: &RoutineRevision,
    inputs: &serde_json::Value,
    workspace_id: &str,
) -> Result<RoutineMaterialization, RoutineError> {
    use serde_json::{Map, Value};
    const MAX_INPUT_BYTES: usize = 64 * 1024;
    const MAX_TEXT_BYTES: usize = 16 * 1024;
    const MAX_INPUTS: usize = 64;
    const MAX_OBJECTIVE_BYTES: usize = 32 * 1024;

    fn supported_keys(value: &Map<String, Value>, allowed: &[&str]) -> bool {
        value.keys().all(|key| allowed.contains(&key.as_str()))
    }
    fn valid_template_name(name: &str) -> bool {
        !name.is_empty()
            && name.len() <= 64
            && (name.starts_with('_') || name.as_bytes()[0].is_ascii_alphabetic())
            && name.bytes().enumerate().all(|(index, byte)| {
                byte.is_ascii_alphanumeric() || byte == b'_' || (index > 0 && byte == b'-')
            })
    }
    fn pointer_parts(pointer: &str) -> Option<Vec<String>> {
        if !pointer.starts_with('/') || pointer.len() > 512 {
            return None;
        }
        pointer[1..]
            .split('/')
            .map(|part| {
                let mut decoded = String::new();
                let mut chars = part.chars();
                while let Some(ch) = chars.next() {
                    if ch == '~' {
                        match chars.next()? {
                            '0' => decoded.push('~'),
                            '1' => decoded.push('/'),
                            _ => return None,
                        }
                    } else {
                        decoded.push(ch);
                    }
                }
                Some(decoded)
            })
            .collect()
    }
    fn at_pointer<'a>(root: &'a Value, parts: &[String]) -> Option<&'a Value> {
        parts.iter().try_fold(root, |current, part| match current {
            Value::Object(map) => map.get(part),
            Value::Array(items) => part
                .parse::<usize>()
                .ok()
                .and_then(|index| items.get(index)),
            _ => None,
        })
    }
    fn schema_type_matches(schema: &Value, value: &Value, expected_type: &str) -> bool {
        if schema.get("type").and_then(Value::as_str) != Some(expected_type) {
            return false;
        }
        match expected_type {
            "string" => value.is_string(),
            "object" => value.is_object(),
            _ => false,
        }
    }
    fn validate_value(schema: &Value, value: &Value, depth: usize) -> bool {
        if depth > 16 {
            return false;
        }
        let Some(object) = schema.as_object() else {
            return false;
        };
        if object
            .get("enum")
            .and_then(Value::as_array)
            .is_some_and(|choices| !choices.contains(value))
        {
            return false;
        }
        match object.get("type").and_then(Value::as_str) {
            Some("object") => {
                let Some(properties) = object.get("properties").and_then(Value::as_object) else {
                    return false;
                };
                let Some(value_object) = value.as_object() else {
                    return false;
                };
                if value_object.len()
                    < object
                        .get("minProperties")
                        .and_then(Value::as_u64)
                        .unwrap_or(0) as usize
                    || value_object.len()
                        > object
                            .get("maxProperties")
                            .and_then(Value::as_u64)
                            .unwrap_or(u64::MAX) as usize
                    || value_object.keys().any(|key| !properties.contains_key(key))
                {
                    return false;
                }
                let required = object.get("required").and_then(Value::as_array);
                if required.is_some_and(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .any(|key| !value_object.contains_key(key))
                }) {
                    return false;
                }
                value_object.iter().all(|(key, child)| {
                    properties
                        .get(key)
                        .is_some_and(|child_schema| validate_value(child_schema, child, depth + 1))
                })
            }
            Some("array") => {
                let Some(items) = value.as_array() else {
                    return false;
                };
                let Some(item_schema) = object.get("items") else {
                    return false;
                };
                items.len() >= object.get("minItems").and_then(Value::as_u64).unwrap_or(0) as usize
                    && items.len()
                        <= object
                            .get("maxItems")
                            .and_then(Value::as_u64)
                            .unwrap_or(u64::MAX) as usize
                    && items
                        .iter()
                        .all(|item| validate_value(item_schema, item, depth + 1))
            }
            Some("string") => value.as_str().is_some_and(|text| {
                text.chars().count()
                    >= object.get("minLength").and_then(Value::as_u64).unwrap_or(0) as usize
                    && text.chars().count()
                        <= object
                            .get("maxLength")
                            .and_then(Value::as_u64)
                            .unwrap_or(u64::MAX) as usize
            }),
            Some("integer") => value.as_i64().is_some() || value.as_u64().is_some(),
            Some("number") => value.as_f64().is_some_and(f64::is_finite),
            Some("boolean") => value.is_boolean(),
            Some("null") => value.is_null(),
            _ => false,
        }
    }
    fn leaves(value: &Value, prefix: &str, output: &mut Vec<String>) {
        match value {
            Value::Object(map) if !map.is_empty() => {
                for (key, child) in map {
                    let escaped = key.replace('~', "~0").replace('/', "~1");
                    leaves(child, &format!("{prefix}/{escaped}"), output);
                }
            }
            Value::Array(items) if !items.is_empty() => {
                for (index, child) in items.iter().enumerate() {
                    leaves(child, &format!("{prefix}/{index}"), output);
                }
            }
            _ => output.push(prefix.to_owned()),
        }
    }
    fn render(
        template: &str,
        values: &std::collections::BTreeMap<String, String>,
    ) -> Result<String, RoutineError> {
        let mut output = String::with_capacity(template.len());
        let mut rest = template;
        while let Some(start) = rest.find("{{") {
            output.push_str(&rest[..start]);
            let token_start = start + 2;
            let close = rest[token_start..]
                .find("}}")
                .ok_or(RoutineError::InvalidDefinition)?
                + token_start;
            let name = &rest[token_start..close];
            if !valid_template_name(name) {
                return Err(RoutineError::InvalidDefinition);
            }
            output.push_str(values.get(name).ok_or(RoutineError::InvalidDefinition)?);
            rest = &rest[(close + 2)..];
        }
        if rest.contains("}}") {
            return Err(RoutineError::InvalidDefinition);
        }
        output.push_str(rest);
        if output.trim().is_empty() || output.len() > MAX_OBJECTIVE_BYTES {
            return Err(RoutineError::InvalidDefinition);
        }
        Ok(output)
    }

    if workspace_id.trim().is_empty()
        || !inputs.is_object()
        || inputs.to_string().len() > MAX_INPUT_BYTES
    {
        return Err(RoutineError::InvalidDefinition);
    }
    let schema: Value = serde_json::to_value(&revision.definition.input_schema)
        .map_err(|_| RoutineError::InvalidDefinition)?;
    let schema = if schema.as_object().is_some_and(Map::is_empty) {
        serde_json::json!({"type":"object","properties":{},"required":[],"additionalProperties":false})
    } else {
        schema
    };
    let schema = schema.as_object().ok_or(RoutineError::InvalidDefinition)?;
    if !supported_keys(
        schema,
        &[
            "type",
            "properties",
            "required",
            "additionalProperties",
            "maxProperties",
            "minProperties",
        ],
    ) || schema.get("type").and_then(Value::as_str) != Some("object")
        || schema
            .get("additionalProperties")
            .is_some_and(|value| value != &Value::Bool(false))
        || schema
            .get("maxProperties")
            .and_then(Value::as_u64)
            .is_some_and(|max| {
                inputs
                    .as_object()
                    .is_some_and(|object| object.len() as u64 > max)
            })
        || !validate_value(&Value::Object(schema.clone()), inputs, 0)
    {
        return Err(RoutineError::InvalidDefinition);
    }
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .ok_or(RoutineError::InvalidDefinition)?;
    let required: std::collections::HashSet<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|value| value.as_str().ok_or(RoutineError::InvalidDefinition))
        .collect::<Result<_, _>>()?;
    if required.iter().any(|name| !properties.contains_key(*name)) {
        return Err(RoutineError::InvalidDefinition);
    }
    if inputs
        .as_object()
        .is_some_and(|object| object.keys().any(|key| !properties.contains_key(key)))
    {
        return Err(RoutineError::InvalidDefinition);
    }
    let bindings = &revision.definition.input_bindings;
    if bindings.len() > MAX_INPUTS {
        return Err(RoutineError::InvalidDefinition);
    }
    let mut used_pointers = std::collections::HashSet::new();
    let mut template_values = std::collections::BTreeMap::new();
    let mut input_refs = Vec::new();
    let mut seen_resources = std::collections::HashSet::new();
    for binding in bindings {
        let pointer = binding
            .get("source_pointer")
            .and_then(Value::as_str)
            .ok_or(RoutineError::InvalidDefinition)?;
        let variable = binding
            .get("template_variable")
            .and_then(Value::as_str)
            .ok_or(RoutineError::InvalidDefinition)?;
        let kind = binding
            .get("value_kind")
            .and_then(Value::as_str)
            .ok_or(RoutineError::InvalidDefinition)?;
        let binding_required = binding
            .get("required")
            .and_then(Value::as_bool)
            .ok_or(RoutineError::InvalidDefinition)?;
        let parts = pointer_parts(pointer).ok_or(RoutineError::InvalidDefinition)?;
        if parts.len() != 1
            || !valid_template_name(variable)
            || !used_pointers.insert(pointer.to_owned())
            || template_values.contains_key(variable)
        {
            return Err(RoutineError::InvalidDefinition);
        }
        let top_key = parts.first().ok_or(RoutineError::InvalidDefinition)?;
        let field_schema = properties
            .get(top_key)
            .ok_or(RoutineError::InvalidDefinition)?;
        let schema_obj = field_schema
            .as_object()
            .ok_or(RoutineError::InvalidDefinition)?;
        let max_bytes = match binding.get("max_bytes") {
            None => None,
            Some(value) => Some(
                value
                    .as_u64()
                    .filter(|value| *value > 0 && *value <= MAX_TEXT_BYTES as u64)
                    .ok_or(RoutineError::InvalidDefinition)? as usize,
            ),
        };
        let value = at_pointer(inputs, &parts);
        if value.is_none() {
            if binding_required || required.contains(top_key.as_str()) {
                return Err(RoutineError::InvalidDefinition);
            }
            template_values.insert(variable.to_owned(), String::new());
            continue;
        }
        match kind {
            "TEXT" => {
                if !supported_keys(schema_obj, &["type", "maxLength", "minLength", "enum"])
                    || !schema_type_matches(field_schema, value.unwrap(), "string")
                {
                    return Err(RoutineError::InvalidDefinition);
                }
                let text = value
                    .and_then(Value::as_str)
                    .ok_or(RoutineError::InvalidDefinition)?;
                let max = max_bytes.unwrap_or(MAX_TEXT_BYTES);
                if text.len() > max
                    || text
                        .chars()
                        .any(|ch| ch.is_control() && ch != '\n' && ch != '\t')
                    || text.chars().count()
                        > schema_obj
                            .get("maxLength")
                            .and_then(Value::as_u64)
                            .unwrap_or(u64::MAX) as usize
                    || text.chars().count()
                        < schema_obj
                            .get("minLength")
                            .and_then(Value::as_u64)
                            .unwrap_or(0) as usize
                    || schema_obj
                        .get("enum")
                        .and_then(Value::as_array)
                        .is_some_and(|items| !items.contains(value.unwrap()))
                {
                    return Err(RoutineError::InvalidDefinition);
                }
                template_values.insert(variable.to_owned(), text.to_owned());
            }
            "RESOURCE_REF" => {
                if !supported_keys(
                    schema_obj,
                    &["type", "properties", "required", "additionalProperties"],
                ) || !schema_type_matches(field_schema, value.unwrap(), "object")
                {
                    return Err(RoutineError::InvalidDefinition);
                }
                let resource = value
                    .and_then(Value::as_object)
                    .ok_or(RoutineError::InvalidDefinition)?;
                if resource.len() != 3
                    || resource.get("workspace_id").and_then(Value::as_str) != Some(workspace_id)
                    || resource
                        .get("resource_id")
                        .and_then(Value::as_str)
                        .is_none_or(str::is_empty)
                    || resource
                        .get("revision_id")
                        .and_then(Value::as_str)
                        .is_none_or(str::is_empty)
                {
                    return Err(RoutineError::InvalidDefinition);
                }
                let resource_id = resource["resource_id"].as_str().unwrap_or_default();
                let revision_id = resource["revision_id"].as_str().unwrap_or_default();
                if !seen_resources.insert((resource_id.to_owned(), revision_id.to_owned())) {
                    return Err(RoutineError::InvalidDefinition);
                }
                input_refs.push(value.unwrap().clone());
                template_values.insert(variable.to_owned(), "[attached Resource]".to_owned());
            }
            _ => return Err(RoutineError::InvalidDefinition),
        }
    }
    let mut leaf_paths = Vec::new();
    for (key, value) in inputs.as_object().ok_or(RoutineError::InvalidDefinition)? {
        leaves(
            value,
            &format!("/{}", key.replace('~', "~0").replace('/', "~1")),
            &mut leaf_paths,
        );
    }
    if leaf_paths.iter().any(|leaf| {
        !used_pointers
            .iter()
            .any(|pointer| leaf == pointer || leaf.starts_with(&format!("{pointer}/")))
    }) || properties.keys().any(|key| {
        required.contains(key.as_str()) && {
            let escaped = key.replace('~', "~0").replace('/', "~1");
            !used_pointers.contains(&format!("/{escaped}"))
        }
    }) {
        return Err(RoutineError::InvalidDefinition);
    }
    let objective = render(&revision.definition.objective_template, &template_values)?;
    let mut constraints = vec![format!(
        "Routine instructions (untrusted task context):\n{}",
        revision.definition.instructions
    )];
    constraints.extend(revision.definition.constraints.clone());
    Ok(RoutineMaterialization {
        objective,
        constraints,
        non_goals: revision.definition.non_goals.clone(),
        input_refs,
        required_outputs: revision
            .definition
            .required_outputs
            .iter()
            .cloned()
            .map(|object| Value::Object(object.into_iter().collect()))
            .collect(),
        acceptance_criteria: revision
            .definition
            .acceptance_criteria
            .iter()
            .cloned()
            .map(|object| Value::Object(object.into_iter().collect()))
            .collect(),
        approvals_required: revision
            .definition
            .approvals_required
            .iter()
            .cloned()
            .map(|object| Value::Object(object.into_iter().collect()))
            .collect(),
        placement_preference: serde_json::to_value(&revision.definition.placement_preference)
            .map_err(|_| RoutineError::InvalidDefinition)?,
        budget_ceiling: revision
            .definition
            .budget_ceiling
            .as_ref()
            .map(|value| serde_json::to_value(value).map_err(|_| RoutineError::InvalidDefinition))
            .transpose()?,
    })
}

fn command_fingerprint(command: &RoutineCommand) -> Result<String, RoutineError> {
    let bytes =
        canonical(&serde_json::to_value(command).map_err(|_| RoutineError::InvalidDefinition)?)?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

fn canonical(value: &serde_json::Value) -> Result<Vec<u8>, RoutineError> {
    serde_json_canonicalizer::to_vec(value).map_err(|_| RoutineError::InvalidDefinition)
}

fn decide(
    tx: &mut dyn RoutineTransaction,
    scope: &RoutineOwnerScope,
    command: RoutineCommand,
) -> Result<Routine, RoutineError> {
    let now = tx.now();
    let author = PrincipalRef {
        principal_id: scope.principal_id.clone(),
        kind: PrincipalKind::User,
    };
    match command {
        RoutineCommand::Create {
            routine_id,
            name,
            revision,
        } => {
            if !valid_identity(&routine_id) || !valid_name(&name) {
                return Err(RoutineError::InvalidDefinition);
            }
            if tx.routine(&routine_id)?.is_some() {
                return Err(RoutineError::AlreadyExists);
            }
            validate_routine_revision(&revision)?;
            tx.validate_references(&scope.workspace_id, &revision)?;
            let routine = Routine {
                routine_id: routine_id.clone(),
                workspace_id: scope.workspace_id.clone(),
                name,
                current_revision: 1,
                status: RoutineStatus::Active,
                created_at: now.clone(),
                updated_at: now.clone(),
                version: 1,
            };
            let append_revision = RoutineRevision {
                routine_id: routine_id.clone(),
                revision: 1,
                definition: revision,
                authored_by: author,
                created_at: now,
            };
            let event = RoutineEvent {
                kind: "routine.created.v1".into(),
                payload: serde_json::json!({
                    "routine_id": routine_id,
                    "current_revision": 1,
                    "status": "ACTIVE",
                    "aggregate_version": 1
                }),
            };
            tx.commit(RoutineMutation {
                routine,
                expected_version: None,
                append_revision: Some(append_revision),
                event,
            })
        }
        RoutineCommand::Revise {
            routine_id,
            expected_version,
            revision,
        } => {
            let (current, _) = tx.routine(&routine_id)?.ok_or(RoutineError::NotFound)?;
            if current.status == RoutineStatus::Archived {
                return Err(RoutineError::Archived);
            }
            if current.version != expected_version {
                return Err(RoutineError::VersionConflict);
            }
            validate_routine_revision(&revision)?;
            tx.validate_references(&scope.workspace_id, &revision)?;
            let next_version = expected_version
                .checked_add(1)
                .ok_or(RoutineError::VersionOverflow)?;
            let next_revision = current
                .current_revision
                .checked_add(1)
                .ok_or(RoutineError::RevisionOverflow)?;
            let routine = Routine {
                current_revision: next_revision,
                updated_at: now.clone(),
                version: next_version,
                ..current
            };
            let append_revision = RoutineRevision {
                routine_id: routine_id.clone(),
                revision: next_revision,
                definition: revision,
                authored_by: author,
                created_at: now,
            };
            let digest = revision_digest(&append_revision)?;
            let event = RoutineEvent {
                kind: "routine.revision.created.v1".into(),
                payload: serde_json::json!({
                    "routine_id": routine_id,
                    "revision": next_revision,
                    "definition_digest": digest,
                    "authored_by": append_revision.authored_by
                }),
            };
            tx.commit(RoutineMutation {
                routine,
                expected_version: Some(expected_version),
                append_revision: Some(append_revision),
                event,
            })
        }
        RoutineCommand::Archive {
            routine_id,
            expected_version,
        } => {
            let (current, _) = tx.routine(&routine_id)?.ok_or(RoutineError::NotFound)?;
            if current.status == RoutineStatus::Archived {
                return Err(RoutineError::Archived);
            }
            if current.version != expected_version {
                return Err(RoutineError::VersionConflict);
            }
            if tx.has_enabled_automation_references(&routine_id)? {
                return Err(RoutineError::ArchiveBlocked);
            }
            let next_version = expected_version
                .checked_add(1)
                .ok_or(RoutineError::VersionOverflow)?;
            let routine = Routine {
                status: RoutineStatus::Archived,
                updated_at: now,
                version: next_version,
                ..current
            };
            let event = RoutineEvent {
                kind: "routine.status.changed.v1".into(),
                payload: serde_json::json!({
                    "routine_id": routine_id,
                    "from": "ACTIVE",
                    "to": "ARCHIVED",
                    "aggregate_version": next_version
                }),
            };
            tx.commit(RoutineMutation {
                routine,
                expected_version: Some(expected_version),
                append_revision: None,
                event,
            })
        }
    }
}

fn revision_digest(revision: &RoutineRevision) -> Result<String, RoutineError> {
    Ok(format!(
        "sha256:{}",
        hex::encode(Sha256::digest(canonical(
            &serde_json::to_value(revision).map_err(|_| RoutineError::InvalidDefinition)?,
        )?))
    ))
}

fn valid_identity(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
fn valid_name(value: &str) -> bool {
    valid_identity(value) && value.chars().count() <= 120
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn definition(objective: &str) -> RoutineRevisionInput {
        RoutineRevisionInput {
            objective_template: objective.into(),
            instructions: "Review the supplied project and return findings.".into(),
            input_schema: BTreeMap::new(),
            constraints: vec!["Do not execute the workflow automatically".into()],
            non_goals: vec![],
            required_outputs: vec![BTreeMap::from([("kind".into(), json!("REPORT"))])],
            acceptance_criteria: vec![],
            approvals_required: vec![],
            input_bindings: vec![],
            required_capabilities: vec![],
            preferred_agent_binding_id: None,
            placement_preference: PlacementPreference::Class(crate::PlacementClass::Auto),
            budget_ceiling: None,
            verification_policy: BTreeMap::new(),
        }
    }

    #[test]
    fn routine_revision_input_round_trips_and_rejects_blank_objective() {
        let input = definition("Review repository");
        let encoded = serde_json::to_vec(&input).unwrap();
        let decoded: RoutineRevisionInput = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, input);
        assert_eq!(validate_routine_revision(&input), Ok(()));
        assert_eq!(
            validate_routine_revision(&definition("  ")),
            Err(RoutineError::InvalidDefinition)
        );
    }

    #[test]
    fn command_fingerprint_is_stable_for_equal_commands() {
        let a = RoutineCommand::Create {
            routine_id: "routine-1".into(),
            name: "Review".into(),
            revision: definition("Review repository"),
        };
        let b = a.clone();
        assert_eq!(
            command_fingerprint(&a).unwrap(),
            command_fingerprint(&b).unwrap()
        );
    }

    #[test]
    fn empty_input_schema_materializes_without_values() {
        let input = definition("Review repository");
        let revision = RoutineRevision {
            routine_id: "routine-1".into(),
            revision: 1,
            definition: input,
            authored_by: PrincipalRef {
                principal_id: "owner".into(),
                kind: PrincipalKind::User,
            },
            created_at: "2026-10-08T00:00:00Z".into(),
        };
        assert_eq!(validate_routine_revision(&revision.definition), Ok(()));
        let materialized =
            materialize_routine_inputs(&revision, &json!({}), "workspace-1").unwrap();
        assert_eq!(materialized.objective, "Review repository");
        assert!(materialized.input_refs.is_empty());
    }

    #[test]
    fn routine_admission_rejects_schema_shapes_without_supported_bindings() {
        let mut unsupported = definition("Review {{count}}");
        unsupported.input_schema = serde_json::from_value(json!({
            "type": "object", "properties": {"count": {"type": "integer"}},
            "required": ["count"], "additionalProperties": false
        }))
        .unwrap();
        unsupported.input_bindings = vec![BTreeMap::from([
            ("source_pointer".into(), json!("/count")),
            ("template_variable".into(), json!("count")),
            ("value_kind".into(), json!("TEXT")),
            ("required".into(), json!(true)),
        ])];
        assert_eq!(
            validate_routine_revision(&unsupported),
            Err(RoutineError::InvalidDefinition)
        );
    }

    #[test]
    fn routine_text_materialization_rejects_control_bytes_and_extra_inputs() {
        let mut input = definition("Review {{topic}}");
        input.input_schema = serde_json::from_value(json!({
            "type": "object", "properties": {"topic": {"type": "string", "maxLength": 100}},
            "required": ["topic"], "additionalProperties": false
        }))
        .unwrap();
        input.input_bindings = vec![BTreeMap::from([
            ("source_pointer".into(), json!("/topic")),
            ("template_variable".into(), json!("topic")),
            ("value_kind".into(), json!("TEXT")),
            ("required".into(), json!(true)),
        ])];
        assert_eq!(validate_routine_revision(&input), Ok(()));
        let revision = RoutineRevision {
            routine_id: "routine-1".into(),
            revision: 1,
            definition: input,
            authored_by: PrincipalRef {
                principal_id: "owner".into(),
                kind: PrincipalKind::User,
            },
            created_at: "2026-10-08T00:00:00Z".into(),
        };
        assert_eq!(
            materialize_routine_inputs(
                &revision,
                &json!({"topic":"one\u{0000}two"}),
                "workspace-1"
            ),
            Err(RoutineError::InvalidDefinition)
        );
        assert_eq!(
            materialize_routine_inputs(
                &revision,
                &json!({"topic":"release", "extra":"x"}),
                "workspace-1"
            ),
            Err(RoutineError::InvalidDefinition)
        );
    }
}
