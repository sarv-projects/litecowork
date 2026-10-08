//! Bounded planning inputs and untrusted native results. No admission authority.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use storage_core::StoreError;
use crate::{PlanningAssignment, ProposedStep, validate_plan_proposal};

const MAX_PLANNING_PACKET_BYTES: usize = 128 * 1024;
const MAX_PLAN_OUTPUT_BYTES: usize = 256 * 1024;

/// Immutable serialization of exact persisted intent. Resource references describe
/// requested inputs and grant no access. This transient value is not the persisted
/// context Resource required for session admission. Never log its text.
#[derive(Clone)]
pub struct TaskPlanningPacket {
    task_id: String,
    task_version: u64,
    task_spec_revision: u64,
    lead_agent_binding_id: String,
    digest: String,
    text: String,
}

impl TaskPlanningPacket {
    pub fn from_assignment(assignment: &PlanningAssignment) -> Result<Self, StoreError> {
        let task = &assignment.task.task;
        let spec = &assignment.task.current_spec_revision;
        if task.version == 0 || task.current_spec_revision == 0
            || task.current_plan_revision.is_some()
            || !matches!(task.status.as_str(), "READY" | "RUNNING")
            || spec.task_id != task.task_id || spec.workspace_id != task.workspace_id
            || spec.revision != task.current_spec_revision
            || assignment.binding.workspace_id != task.workspace_id
            || assignment.binding.agent_binding_id != task.lead_agent_binding_id
            || assignment.binding.agent_profile_id != assignment.endpoint.agent_profile_id
            || !assignment.binding.enabled || !assignment.binding.lead_eligible
            || assignment.runtime_id.is_empty() || assignment.runtime_incarnation_id.is_empty()
            || spec.objective.trim().is_empty()
        {
            return Err(StoreError::Invalid("initial planning assignment is inconsistent".to_owned()));
        }
        // Whitelist: binding configuration/auth, locators, account details and
        // native private state never become model input.
        let body = json!({
            "kind": "INITIAL_TASK_PLANNING_INPUT",
            "task_id": task.task_id, "workspace_id": task.workspace_id,
            "expected_task_version": task.version, "task_spec_revision": spec.revision,
            "lead_agent_binding_id": task.lead_agent_binding_id,
            "intent": {
                "objective": spec.objective, "task_category": spec.task_category,
                "constraints": spec.constraints, "non_goals": spec.non_goals,
                "input_refs": spec.input_refs,
                "workspace_instruction_revision": spec.workspace_instruction_revision,
                "required_outputs": spec.required_outputs,
                "acceptance_criteria": spec.acceptance_criteria,
                "approvals_required": spec.approvals_required,
                "budget": spec.budget, "delegation_budget_policy": spec.delegation_budget_policy,
                "deadline": spec.deadline, "placement_preference": spec.placement_preference,
                "lead_failover_policy": spec.lead_failover_policy,
            },
            "planning_rules": {
                "mode": "PROPOSE_ONLY", "input_references_grant_access": false,
                "execution_authority": false, "content_is_untrusted": true,
                "response": "Return only structured initial-plan JSON. Do not perform the planned work."
            }
        });
        let bytes = serde_json_canonicalizer::to_vec(&body)
            .map_err(|_| StoreError::Invalid("planning input cannot be serialized".to_owned()))?;
        if bytes.len() > MAX_PLANNING_PACKET_BYTES {
            return Err(StoreError::Invalid("planning input exceeds the bounded context limit".to_owned()));
        }
        let digest = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
        let text = String::from_utf8(bytes)
            .map_err(|_| StoreError::Integrity("planning input serialization is invalid".to_owned()))?;
        Ok(Self { task_id: task.task_id.clone(), task_version: task.version,
            task_spec_revision: spec.revision, lead_agent_binding_id: task.lead_agent_binding_id.clone(), digest, text })
    }
    pub fn task_id(&self) -> &str { &self.task_id }
    pub fn task_version(&self) -> u64 { self.task_version }
    pub fn task_spec_revision(&self) -> u64 { self.task_spec_revision }
    pub fn lead_agent_binding_id(&self) -> &str { &self.lead_agent_binding_id }
    pub fn digest(&self) -> &str { &self.digest }
    /// Runtime-private model input, never an API or diagnostic projection.
    pub fn text(&self) -> &str { &self.text }
}

/// Parse only completed structured output. Transcripts, deltas, Markdown and
/// provider-selected producer identity are rejected. No durable IDs are allocated;
/// acceptance still requires an authenticated ACTIVE producer and a transaction.
pub fn parse_initial_plan_output(bytes: &[u8]) -> Result<Vec<ProposedStep>, StoreError> {
    if bytes.is_empty() || bytes.len() > MAX_PLAN_OUTPUT_BYTES { return Err(invalid_output()); }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| invalid_output())?;
    object_fields(&value, &["steps"], &[])?;
    let steps = array(&value, "steps", 100)?;
    if steps.is_empty() { return Err(invalid_output()); }
    let proposed = steps.iter().map(|step| {
        object_fields(step, &["logical_key", "title", "objective", "depends_on_logical_keys", "required_capabilities", "acceptance_criteria"], &[])?;
        let dependencies = array(step, "depends_on_logical_keys", 100)?.iter()
            .map(|key| bounded_text(key, 128).map(str::to_owned)).collect::<Result<Vec<_>, _>>()?;
        let capabilities = array(step, "required_capabilities", 32)?;
        let criteria = array(step, "acceptance_criteria", 32)?;
        Ok(ProposedStep {
            logical_key: bounded_text(&step["logical_key"], 128)?.to_owned(),
            title: bounded_text(&step["title"], 512)?.to_owned(),
            objective: bounded_text(&step["objective"], 16 * 1024)?.to_owned(),
            depends_on_logical_keys: dependencies,
            required_capabilities: capabilities.clone(), acceptance_criteria: criteria.clone(),
        })
    }).collect::<Result<Vec<_>, StoreError>>()?;
    let ids = (0..proposed.len()).map(|index| format!("validation_{index}")).collect::<Vec<_>>();
    validate_plan_proposal(&proposed, &ids).map_err(|_| invalid_output())?;
    Ok(proposed)
}

fn invalid_output() -> StoreError {
    // Do not echo untrusted output or serde error excerpts to the owner/log.
    StoreError::Invalid("native initial plan output is invalid or exceeds a bound".to_owned())
}
fn object_fields(value: &Value, required: &[&str], optional: &[&str]) -> Result<(), StoreError> {
    let object = value.as_object().ok_or_else(invalid_output)?;
    if required.iter().any(|field| !object.contains_key(*field))
        || object.keys().any(|field| !required.contains(&field.as_str()) && !optional.contains(&field.as_str())) { return Err(invalid_output()); }
    Ok(())
}
fn array<'a>(value: &'a Value, key: &str, maximum: usize) -> Result<&'a Vec<Value>, StoreError> {
    value.get(key).and_then(Value::as_array).filter(|array| array.len() <= maximum).ok_or_else(invalid_output)
}
fn bounded_text(value: &Value, maximum: usize) -> Result<&str, StoreError> {
    value.as_str().filter(|text| !text.trim().is_empty() && text.len() <= maximum && !text.chars().any(char::is_control)).ok_or_else(invalid_output)
}

/// Shared by parsing and the public service, so typed callers cannot bypass bounds.
pub(super) fn validate_plan_step_content(step: &ProposedStep) -> Result<(), StoreError> {
    if step.depends_on_logical_keys.len() > 100
        || step.depends_on_logical_keys.iter().any(|key| key.is_empty() || key.len() > 128 || key.chars().any(char::is_control))
        || step.required_capabilities.len() > 32 || step.acceptance_criteria.len() > 32
    { return Err(invalid_output()); }
    for capability in &step.required_capabilities {
        object_fields(capability, &["semantic_requirement", "operation_ids"], &["resource_scope"])?;
        bounded_text(&capability["semantic_requirement"], 1024)?;
        validate_operations(array(capability, "operation_ids", 32)?)?;
        if let Some(scope) = capability.get("resource_scope") {
            validate_resource_scope(scope)?;
        }
    }
    for criterion in &step.acceptance_criteria {
        object_fields(criterion, &["criterion_id", "description", "required_evidence", "mandatory"], &["verifier_hint", "subject_refs"])?;
        bounded_text(&criterion["criterion_id"], 128)?;
        bounded_text(&criterion["description"], 2048)?;
        if !matches!(criterion["required_evidence"].as_str(), Some("REPORTED" | "OBSERVED" | "VERIFIED"))
            || !criterion["mandatory"].is_boolean() { return Err(invalid_output()); }
        if let Some(hint) = criterion.get("verifier_hint").filter(|hint| !hint.is_null()) {
            if !hint.as_str().is_some_and(|text| text.len() <= 1024 && !text.chars().any(char::is_control)) { return Err(invalid_output()); }
        }
        if criterion.get("subject_refs").is_some() {
            for reference in array(criterion, "subject_refs", 32)? { validate_resource_ref(reference)?; }
        }
    }
    Ok(())
}

fn validate_operations(operations: &[Value]) -> Result<(), StoreError> {
    let mut seen = std::collections::HashSet::new();
    for operation in operations {
        if !seen.insert(bounded_text(operation, 128)?) { return Err(invalid_output()); }
    }
    Ok(())
}

/// SCHEMAS.md ResourceRef and Operator ResourceRef: logical identity only. An
/// optional null revision follows the Operator shape; it is never a pinned input.
fn validate_resource_ref(reference: &Value) -> Result<(), StoreError> {
    object_fields(reference, &["workspace_id", "resource_id"], &["revision_id"])?;
    bounded_text(&reference["workspace_id"], 256)?;
    bounded_text(&reference["resource_id"], 256)?;
    if let Some(revision) = reference.get("revision_id").filter(|revision| !revision.is_null()) {
        bounded_text(revision, 256)?;
    }
    Ok(())
}

/// Canonical ResourceScope fields are optional in the machine/API shape. Unknown
/// locator/authority fields and null scopes are refused; constraints are bounded
/// JSON data, not executable policy or proof that a Resource may be accessed.
fn validate_resource_scope(scope: &Value) -> Result<(), StoreError> {
    object_fields(scope, &[], &["resource_refs", "operation_ids", "constraints"])?;
    if scope.get("resource_refs").is_some() {
        for reference in array(scope, "resource_refs", 32)? { validate_resource_ref(reference)?; }
    }
    if scope.get("operation_ids").is_some() { validate_operations(array(scope, "operation_ids", 32)?)?; }
    if let Some(constraints) = scope.get("constraints") {
        if !constraints.is_object() { return Err(invalid_output()); }
        let mut remaining_nodes = 1024;
        validate_constraint_json(constraints, 0, &mut remaining_nodes)?;
        if serde_json::to_vec(constraints).map_err(|_| invalid_output())?.len() > 16 * 1024 { return Err(invalid_output()); }
    }
    Ok(())
}

fn validate_constraint_json(value: &Value, depth: usize, remaining_nodes: &mut usize) -> Result<(), StoreError> {
    if depth > 8 || *remaining_nodes == 0 { return Err(invalid_output()); }
    *remaining_nodes -= 1;
    match value {
        Value::Object(object) => {
            if object.len() > 64 { return Err(invalid_output()); }
            for (key, child) in object {
                if key.is_empty() || key.len() > 128 || key.chars().any(char::is_control) { return Err(invalid_output()); }
                validate_constraint_json(child, depth + 1, remaining_nodes)?;
            }
        }
        Value::Array(array) => {
            if array.len() > 32 { return Err(invalid_output()); }
            for child in array { validate_constraint_json(child, depth + 1, remaining_nodes)?; }
        }
        Value::String(text) if text.len() > 2048 || text.chars().any(char::is_control) => return Err(invalid_output()),
        _ => {},
    }
    Ok(())
}

pub(super) fn plan_step_serialized_size(step: &ProposedStep) -> Result<usize, StoreError> {
    // Called only after structural and recursive bounds above. Include commas so
    // the total plan bound is conservative even for typed service callers.
    serde_json::to_vec(&json!({
        "logical_key":step.logical_key, "title":step.title, "objective":step.objective,
        "depends_on_logical_keys":step.depends_on_logical_keys,
        "required_capabilities":step.required_capabilities, "acceptance_criteria":step.acceptance_criteria,
    })).map(|bytes| bytes.len() + 1).map_err(|_| invalid_output())
}

pub fn initial_plan_output_schema() -> Value {
    json!({
        "type":"object",
        "additionalProperties":false,
        "required":["steps"],
        "properties":{
            "steps":{
                "type":"array",
                "minItems":1,
                "maxItems":100,
                "items":{
                    "type":"object",
                    "additionalProperties":false,
                    "required":["logical_key","title","objective","depends_on_logical_keys","required_capabilities","acceptance_criteria"],
                    "properties":{
                        "logical_key":{"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9._-]+$"},
                        "title":{"type":"string","minLength":1,"maxLength":512},
                        "objective":{"type":"string","minLength":1,"maxLength":16384},
                        "depends_on_logical_keys":{
                            "type":"array","maxItems":100,"uniqueItems":true,
                            "items":{"type":"string","minLength":1,"maxLength":128}
                        },
                        "required_capabilities":{
                            "type":"array","maxItems":32,
                            "items":{
                                "type":"object","additionalProperties":false,
                                "required":["semantic_requirement","operation_ids"],
                                "properties":{
                                    "semantic_requirement":{"type":"string","minLength":1,"maxLength":1024},
                                    "operation_ids":{"type":"array","maxItems":32,"uniqueItems":true,"items":{"type":"string","minLength":1,"maxLength":128}},
                                    "resource_scope":resource_scope_output_schema()
                                }
                            }
                        },
                        "acceptance_criteria":{
                            "type":"array","maxItems":32,
                            "items":{
                                "type":"object","additionalProperties":false,
                                "required":["criterion_id","description","required_evidence","mandatory"],
                                "properties":{
                                    "criterion_id":{"type":"string","minLength":1,"maxLength":128},
                                    "description":{"type":"string","minLength":1,"maxLength":2048},
                                    "required_evidence":{"type":"string","enum":["REPORTED","OBSERVED","VERIFIED"]},
                                    "verifier_hint":{"type":["string","null"],"maxLength":1024},
                                    "mandatory":{"type":"boolean"},
                                    "subject_refs":{"type":"array","maxItems":32,"items":resource_ref_output_schema()}
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}

fn resource_ref_output_schema() -> Value {
    json!({"type":"object", "additionalProperties":false, "required":["workspace_id", "resource_id"],
        "properties":{
            "workspace_id":{"type":"string", "minLength":1, "maxLength":256},
            "resource_id":{"type":"string", "minLength":1, "maxLength":256},
            "revision_id":{"type":["string", "null"], "minLength":1, "maxLength":256}
        }})
}
fn resource_scope_output_schema() -> Value {
    json!({"type":"object", "additionalProperties":false,
        "properties":{
            "resource_refs":{"type":"array", "maxItems":32, "items":resource_ref_output_schema()},
            "operation_ids":{"type":"array", "maxItems":32, "uniqueItems":true, "items":{"type":"string", "minLength":1, "maxLength":128}},
            "constraints":{"type":"object", "maxProperties":64}
        }})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> Value {
        json!({"steps":[{
            "logical_key":"review", "title":"Review requested inputs", "objective":"Read the scoped Resource revisions",
            "depends_on_logical_keys":[], "required_capabilities":[],
            "acceptance_criteria":[{"criterion_id":"reviewed", "description":"Provide findings", "required_evidence":"REPORTED", "mandatory":true}]
        }]})
    }

    fn parse(value: &Value) -> Result<Vec<ProposedStep>, StoreError> {
        parse_initial_plan_output(&serde_json::to_vec(value).unwrap())
    }

    #[test]
    fn parses_structured_plan_without_allocating_durable_identities() {
        let parsed = parse(&plan()).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].logical_key, "review");
    }

    #[test]
    fn rejects_provider_authority_and_transcript_fields() {
        for key in ["task_id", "agent_session_id", "expected_task_version", "reason_for_revision", "transcript"] {
            let mut value = plan();
            value[key] = json!("untrusted");
            assert!(parse(&value).is_err());
        }
        let mut value = plan();
        value["steps"][0]["step_id"] = json!("injected");
        assert!(parse(&value).is_err());
        assert!(parse_initial_plan_output(b"```json\n{}\n```").is_err());
    }

    #[test]
    fn rejects_cycles_missing_dependencies_and_duplicate_keys() {
        let mut value = plan();
        value["steps"][0]["depends_on_logical_keys"] = json!(["review"]);
        assert!(parse(&value).is_err());
        value["steps"][0]["depends_on_logical_keys"] = json!(["missing"]);
        assert!(parse(&value).is_err());
        let mut value = plan();
        let duplicate = value["steps"][0].clone();
        value["steps"].as_array_mut().unwrap().push(duplicate);
        assert!(parse(&value).is_err());
        value["steps"][1]["logical_key"] = json!("second");
        value["steps"][0]["depends_on_logical_keys"] = json!(["second"]);
        value["steps"][1]["depends_on_logical_keys"] = json!(["review"]);
        assert!(parse(&value).is_err());
    }

    #[test]
    fn rejects_oversized_output_and_invalid_evidence() {
        assert!(parse_initial_plan_output(&vec![b' '; MAX_PLAN_OUTPUT_BYTES + 1]).is_err());
        let mut value = plan();
        value["steps"][0]["acceptance_criteria"][0]["required_evidence"] = json!("EXECUTED");
        assert!(parse(&value).is_err());
        value = plan();
        value["steps"][0]["title"] = json!("a".repeat(513));
        assert!(parse(&value).is_err());
        value = plan();
        value["steps"][0]["required_capabilities"] = json!([{
            "semantic_requirement":"filesystem.read", "operation_ids":["read"], "credentials":"forbidden"
        }]);
        assert!(parse(&value).is_err());
    }

    fn validate_typed(steps: &[ProposedStep]) -> Result<(), StoreError> {
        let ids = (0..steps.len()).map(|index| format!("step_{index}")).collect::<Vec<_>>();
        validate_plan_proposal(steps, &ids)
    }

    #[test]
    fn typed_submission_cannot_bypass_parser_bounds() {
        let base = parse(&plan()).unwrap();
        for field in ["dependencies", "capabilities", "criteria", "operations", "subjects"] {
            let mut steps = base.clone();
            match field {
                "dependencies" => steps[0].depends_on_logical_keys = vec!["review".to_owned(); 101],
                "capabilities" => steps[0].required_capabilities = vec![json!({"semantic_requirement":"read", "operation_ids":[]}); 33],
                "criteria" => steps[0].acceptance_criteria = vec![steps[0].acceptance_criteria[0].clone(); 33],
                "operations" => steps[0].required_capabilities = vec![json!({"semantic_requirement":"read", "operation_ids":vec!["read"; 33]})],
                "subjects" => steps[0].acceptance_criteria[0]["subject_refs"] = json!(vec![json!({"workspace_id":"workspace", "resource_id":"resource"}); 33]),
                _ => unreachable!(),
            }
            assert!(validate_typed(&steps).is_err(), "typed {field} bypassed bounds");
        }
        let mut large = Vec::new();
        for index in 0..17 {
            let mut step = base[0].clone();
            step.logical_key = format!("step_{index}");
            step.objective = "x".repeat(16 * 1024);
            large.push(step);
        }
        assert!(validate_typed(&large).is_err());
    }

    #[test]
    fn resource_refs_and_scopes_use_logical_identity_shapes() {
        let reference = json!({"workspace_id":"workspace", "resource_id":"resource", "revision_id":"revision"});
        let mut value = plan();
        value["steps"][0]["required_capabilities"] = json!([{
            "semantic_requirement":"read", "operation_ids":["read"],
            "resource_scope":{"resource_refs":[reference.clone()], "operation_ids":["read"], "constraints":{"selection":"explicit"}}
        }]);
        value["steps"][0]["acceptance_criteria"][0]["subject_refs"] = json!([reference]);
        assert!(parse(&value).is_ok());
        let valid = value.clone();
        for invalid in [json!({}), json!({"resource_id":"resource"}), json!({"workspace_id":"workspace", "resource_id":"resource", "path":"/private"}), json!({"workspace_id":"workspace", "resource_id":"resource", "revision_id":4})] {
            value = valid.clone();
            value["steps"][0]["acceptance_criteria"][0]["subject_refs"] = json!([invalid.clone()]);
            assert!(parse(&value).is_err());
            value = valid.clone();
            value["steps"][0]["required_capabilities"][0]["resource_scope"]["resource_refs"] = json!([invalid]);
            assert!(parse(&value).is_err());
        }
        for invalid_scope in [Value::Null, json!({"root":"/private"}), json!({"constraints":[]}), json!({"operation_ids":["read", "read"]})] {
            let mut steps = parse(&valid).unwrap();
            steps[0].required_capabilities[0]["resource_scope"] = invalid_scope;
            assert!(validate_typed(&steps).is_err());
        }
    }

    #[test]
    fn nested_constraints_remain_bounded_for_typed_callers() {
        let mut nested = json!(true);
        for _ in 0..10 { nested = json!({"child":nested}); }
        let mut steps = parse(&plan()).unwrap();
        steps[0].required_capabilities = vec![json!({
            "semantic_requirement":"read", "operation_ids":[], "resource_scope":{"constraints":nested}
        })];
        assert!(validate_typed(&steps).is_err());
        steps[0].required_capabilities[0]["resource_scope"]["constraints"] = json!({"value":"x".repeat(2049)});
        assert!(validate_typed(&steps).is_err());
        steps[0].required_capabilities[0]["resource_scope"]["constraints"] = json!({"values":vec![true; 33]});
        assert!(validate_typed(&steps).is_err());
    }

    #[test]
    fn errors_never_echo_provider_text() {
        let error = parse_initial_plan_output(b"private-provider-token").unwrap_err();
        assert!(!format!("{error:?}").contains("private-provider-token"));
    }
}
