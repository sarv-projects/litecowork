use crate::{ValidationError, blocks::closed_object};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const MAX_AST_NODES: usize = 64;
const MAX_AST_DEPTH: usize = 12;

pub(crate) fn validate_calc_ast(
    ast: &Value,
    declared_inputs: &BTreeSet<String>,
) -> Result<(), ValidationError> {
    let mut count = 0usize;
    validate_node(ast, declared_inputs, 0, &mut count)
}

/// Evaluate a previously validated AST with the exact initial input values.
/// The AST bound limits recursion to 13 levels and 64 nodes.
pub(crate) fn evaluate_calc_ast(
    ast: &Value,
    values: &BTreeMap<String, f64>,
) -> Result<f64, ValidationError> {
    let declared_inputs = values.keys().cloned().collect();
    validate_calc_ast(ast, &declared_inputs)?;
    evaluate_node(ast, values)
}

fn evaluate_node(node: &Value, values: &BTreeMap<String, f64>) -> Result<f64, ValidationError> {
    let value = match node["kind"].as_str() {
        Some("LITERAL") => node["value"].as_f64().ok_or(ValidationError::Invalid(
            "calculator literal must be numeric",
        ))?,
        Some("VARIABLE") => {
            let id = node["input_id"]
                .as_str()
                .ok_or(ValidationError::Invalid("calculator variable id required"))?;
            *values.get(id).ok_or(ValidationError::Invalid(
                "calculator input value is unavailable",
            ))?
        }
        Some("BINARY_OP") => {
            let left = evaluate_node(&node["left"], values)?;
            let right = evaluate_node(&node["right"], values)?;
            match node["operator"].as_str() {
                Some("+") => left + right,
                Some("-") => left - right,
                Some("*") => left * right,
                Some("/") if right != 0.0 => left / right,
                Some("%") if right != 0.0 => left % right,
                Some("/" | "%") => {
                    return Err(ValidationError::Invalid("calculator division by zero"));
                }
                _ => return Err(ValidationError::Invalid("unsupported calculator operator")),
            }
        }
        Some("UNARY_OP") => -evaluate_node(&node["operand"], values)?,
        Some("FUNCTION") => {
            let function = node["function"]
                .as_str()
                .ok_or(ValidationError::Invalid("calculator function required"))?;
            let arguments = node["arguments"]
                .as_array()
                .ok_or(ValidationError::Invalid(
                    "calculator function arguments required",
                ))?;
            match function {
                "MIN" | "MAX" => {
                    let mut result = evaluate_node(&arguments[0], values)?;
                    for argument in &arguments[1..] {
                        let next = evaluate_node(argument, values)?;
                        result = if function == "MIN" {
                            result.min(next)
                        } else {
                            result.max(next)
                        };
                    }
                    result
                }
                "ABS" => evaluate_node(&arguments[0], values)?.abs(),
                "ROUND" => {
                    let input = evaluate_node(&arguments[0], values)?;
                    let precision = if arguments.len() == 2 {
                        arguments[1]["value"]
                            .as_f64()
                            .ok_or(ValidationError::Invalid(
                                "ROUND precision must be a numeric literal",
                            ))? as i32
                    } else {
                        0
                    };
                    let factor = 10_f64.powi(precision);
                    (input * factor).round() / factor
                }
                "CLAMP" => {
                    let input = evaluate_node(&arguments[0], values)?;
                    let lower = evaluate_node(&arguments[1], values)?;
                    let upper = evaluate_node(&arguments[2], values)?;
                    if lower > upper {
                        return Err(ValidationError::Invalid(
                            "CLAMP lower bound exceeds upper bound",
                        ));
                    }
                    input.clamp(lower, upper)
                }
                _ => return Err(ValidationError::Invalid("unsupported calculator function")),
            }
        }
        _ => return Err(ValidationError::Invalid("unsupported calculator AST node")),
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err(ValidationError::Invalid("calculator result is not finite"))
    }
}

fn validate_node(
    node: &Value,
    declared_inputs: &BTreeSet<String>,
    depth: usize,
    count: &mut usize,
) -> Result<(), ValidationError> {
    *count += 1;
    if *count > MAX_AST_NODES || depth > MAX_AST_DEPTH {
        return Err(ValidationError::Invalid(
            "calculator AST complexity limit exceeded",
        ));
    }
    match node["kind"].as_str() {
        Some("LITERAL") => {
            closed_object(node, &["kind", "value"], &[])?;
            if !node["value"].as_f64().is_some_and(f64::is_finite) {
                return Err(ValidationError::Invalid(
                    "calculator literal must be finite",
                ));
            }
        }
        Some("VARIABLE") => {
            closed_object(node, &["kind", "input_id"], &[])?;
            let id = node["input_id"]
                .as_str()
                .ok_or(ValidationError::Invalid("calculator variable id required"))?;
            if !declared_inputs.contains(id) {
                return Err(ValidationError::Invalid(
                    "calculator variable is not a declared input",
                ));
            }
        }
        Some("BINARY_OP") => {
            closed_object(node, &["kind", "operator", "left", "right"], &[])?;
            if !matches!(node["operator"].as_str(), Some("+" | "-" | "*" | "/" | "%")) {
                return Err(ValidationError::Invalid("unsupported calculator operator"));
            }
            validate_node(&node["left"], declared_inputs, depth + 1, count)?;
            validate_node(&node["right"], declared_inputs, depth + 1, count)?;
        }
        Some("UNARY_OP") => {
            closed_object(node, &["kind", "operator", "operand"], &[])?;
            if node["operator"] != "-" {
                return Err(ValidationError::Invalid(
                    "unsupported unary calculator operator",
                ));
            }
            validate_node(&node["operand"], declared_inputs, depth + 1, count)?;
        }
        Some("FUNCTION") => validate_function(node, declared_inputs, depth, count)?,
        _ => return Err(ValidationError::Invalid("unsupported calculator AST node")),
    }
    Ok(())
}

fn validate_function(
    node: &Value,
    declared_inputs: &BTreeSet<String>,
    depth: usize,
    count: &mut usize,
) -> Result<(), ValidationError> {
    closed_object(node, &["kind", "function", "arguments"], &[])?;
    let function = node["function"]
        .as_str()
        .ok_or(ValidationError::Invalid("calculator function required"))?;
    let arguments = node["arguments"]
        .as_array()
        .ok_or(ValidationError::Invalid(
            "calculator function arguments required",
        ))?;
    let valid_arity = match function {
        "MIN" | "MAX" => (2..=8).contains(&arguments.len()),
        "ROUND" => (1..=2).contains(&arguments.len()),
        "ABS" => arguments.len() == 1,
        "CLAMP" => arguments.len() == 3,
        _ => false,
    };
    if !valid_arity {
        return Err(ValidationError::Invalid(
            "invalid calculator function arity",
        ));
    }
    if function == "ROUND" && arguments.len() == 2 {
        let precision = &arguments[1];
        if precision["kind"] != "LITERAL"
            || precision["value"].as_f64().is_none_or(|value| {
                !value.is_finite() || value.fract() != 0.0 || !(-6.0..=6.0).contains(&value)
            })
        {
            return Err(ValidationError::Invalid(
                "ROUND precision must be an integer literal from -6 through 6",
            ));
        }
    }
    if function == "CLAMP" {
        let lower = literal_value(&arguments[1]);
        let upper = literal_value(&arguments[2]);
        if lower.zip(upper).is_some_and(|(lower, upper)| lower > upper) {
            return Err(ValidationError::Invalid(
                "CLAMP lower bound exceeds upper bound",
            ));
        }
    }
    for argument in arguments {
        validate_node(argument, declared_inputs, depth + 1, count)?;
    }
    Ok(())
}

fn literal_value(node: &Value) -> Option<f64> {
    (node["kind"] == "LITERAL")
        .then(|| node["value"].as_f64())
        .flatten()
        .filter(|value| value.is_finite())
}
