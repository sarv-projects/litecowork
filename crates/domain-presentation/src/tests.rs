use super::blocks::{es_number_string, table_matches_semantic_markdown};
use super::*;

#[test]
fn semantic_markdown_table_extraction_preserves_exact_utf8_byte_ranges() {
    let semantic = "Intro 🌍\n\n| Name | Count |\n| --- | --- |\n| A | 1 |\n| B | 2 |\n\nTail";
    let tables = extract_semantic_markdown_tables(semantic);
    assert_eq!(tables.len(), 1);
    let table = &tables[0];
    assert_eq!(
        &semantic.as_bytes()[table.start_utf8_byte..table.end_utf8_byte_exclusive],
        b"| Name | Count |\n| --- | --- |\n| A | 1 |\n| B | 2 |"
    );
    assert_eq!(table.headers, ["Name", "Count"]);
    assert_eq!(table.rows, [vec!["A", "1"], vec!["B", "2"]]);
}

#[test]
fn semantic_markdown_table_extraction_handles_crlf_and_rejects_ambiguous_rows() {
    let semantic = "Before\r\n| A | B |\r\n| --- | --- |\r\n| x | y |\r\nAfter\r\n| C | D |\r\n| --- | --- |\r\n| escaped\\|pipe | value |\r\n";
    let tables = extract_semantic_markdown_tables(semantic);
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].headers, ["A", "B"]);
    assert_eq!(tables[0].rows, [vec!["x", "y"]]);
    assert!(semantic.is_char_boundary(tables[0].start_utf8_byte));
    assert!(semantic.is_char_boundary(tables[0].end_utf8_byte_exclusive));
}
use serde_json::{Value, json};

fn slice(text: &str, selected: &str) -> Value {
    let start = text.find(selected).unwrap();
    let end = start + selected.len();
    json!({
        "start_utf8_byte": start,
        "end_utf8_byte_exclusive": end,
        "slice_digest": digest(&text.as_bytes()[start..end])
    })
}

fn document(renderer_version: u32, semantic_text: &str, blocks: Vec<Value>) -> Vec<u8> {
    fn collect_provenance(block: &Value, path: &str, provenance: &mut Vec<Value>) {
        provenance.push(json!({
            "block_path": path,
            "origin": "SEMANTIC_MESSAGE",
            "resource_refs": [],
            "artifact_refs": [],
            "evidence_refs": [],
            "verification_refs": []
        }));
        if let Some(children) = block["children"].as_array() {
            for (index, child) in children.iter().enumerate() {
                collect_provenance(child, &format!("{path}/children/{index}"), provenance);
            }
        }
    }
    let mut provenance = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        collect_provenance(block, &format!("/{index}"), &mut provenance);
    }
    let value = json!({
        "schema_version": 1,
        "renderer_contract_version": renderer_version,
        "presentation_id": "presentation-test",
        "message_id": "message-test",
        "semantic_content_digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "root_blocks": blocks,
        "block_provenance": provenance
    });
    let _ = semantic_text;
    serde_json_canonicalizer::to_vec(&value).unwrap()
}

fn validate(
    renderer_version: u32,
    semantic_text: &str,
    bytes: &[u8],
) -> Result<(), ValidationError> {
    validate_document(
        ValidationContext {
            schema_version: 1,
            renderer_contract_version: renderer_version,
            presentation_id: "presentation-test",
            message_id: "message-test",
            semantic_content_digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            document_digest: &digest(bytes),
            document_size_bytes: bytes.len() as u64,
            semantic_text,
        },
        bytes,
    )
}

#[test]
fn renderer_v1_accepts_semantic_text_slice_and_rejects_v2_block() {
    let text = "Hello 🌍";
    let good = document(
        1,
        text,
        vec![json!({"kind":"TEXT_SLICE","source":slice(text, text)})],
    );
    assert_eq!(validate(1, text, &good), Ok(()));

    let metric = json!({
        "kind":"METRIC",
        "binding":{
            "source":"SEMANTIC_TABLE",
            "table_slice":slice(text, text),
            "row_index":0,
            "column_index":0
        }
    });
    let unsupported = document(1, text, vec![metric]);
    assert_eq!(
        validate(1, text, &unsupported),
        Err(ValidationError::UnsupportedBlock)
    );
}

#[test]
fn metric_and_chart_are_bound_to_exact_numeric_table_cells() {
    let text = "Revenue for Feb is reported in INR at lakh scale for the monthly interval with sum aggregation.\n\nRevenue by Month is reported in INR at lakh scale for the monthly interval with sum aggregation.\n\n| Month | Revenue |
| --- | --- |
| Jan | 12 |
| Feb | 18 |";
    let table = "| Month | Revenue |
| --- | --- |
| Jan | 12 |
| Feb | 18 |";
    let metric = json!({
        "kind":"METRIC",
        "binding":{
            "source":"SEMANTIC_TABLE",
            "table_slice":slice(text, table),
            "row_index":1,
            "column_index":1
        },
        "semantics":{"unit":"INR","scale":"LAKH","time_interval":"monthly","aggregation":"SUM"},
        "accessible_summary":"Revenue for Feb is reported in INR at lakh scale for the monthly interval with sum aggregation."
    });
    let chart = json!({
        "kind":"CHART",
        "chart_kind":"LINE",
        "binding":{"source":"SEMANTIC_TABLE","semantic_table_slice":slice(text, table)},
        "category_column_index":0,
        "series_column_indexes":[1],
        "accessible_summary":"Revenue by Month is reported in INR at lakh scale for the monthly interval with sum aggregation.",
        "semantics":{"unit":"INR","scale":"LAKH","time_interval":"monthly","aggregation":"SUM"}
    });
    let bytes = document(2, text, vec![metric, chart.clone()]);
    assert_eq!(validate(2, text, &bytes), Ok(()));

    let mut invalid_chart = chart;
    invalid_chart["series_column_indexes"] = json!([0]);
    let invalid = document(2, text, vec![invalid_chart]);
    assert!(validate(2, text, &invalid).is_err());
}

#[test]
fn calculator_checks_initial_values_labels_format_and_ast() {
    let text = "Amount\n| Input | Value |
| --- | --- |
| Amount | 12 |
| Years | 4 |
| Estimated amount | 24 |
The estimated amount is shown below.";
    let table = "| Input | Value |
| --- | --- |
| Amount | 12 |
| Years | 4 |";
    let result_table = "| Input | Value |
| --- | --- |
| Amount | 12 |
| Years | 4 |
| Estimated amount | 24 |";
    let calculator = json!({
        "kind":"SAFE_CALCULATOR",
        "inputs":[{
            "id":"amount",
            "label":"Amount",
            "binding":{
                "source":"SEMANTIC_TABLE",
                "table_slice":slice(text, table),
                "row_index":0,
                "column_index":1
            },
            "min_value":0,
            "max_value":100,
            "step":1
        }],
        "formula_ast":{"kind":"BINARY_OP","operator":"*","left":{"kind":"VARIABLE","input_id":"amount"},"right":{"kind":"LITERAL","value":2}},
        "result_binding":{
            "source":"SEMANTIC_TABLE",
            "table_slice":slice(text, result_table),
            "row_index":2,
            "column_index":1
        },
        "output_format":{"style":"NUMBER"},
        "fallback_text":"The estimated amount is shown below."
    });
    let good = document(2, text, vec![calculator.clone()]);
    assert_eq!(validate(2, text, &good), Ok(()));

    let mut mismatched_result = calculator.clone();
    mismatched_result["result_binding"]["row_index"] = json!(0);
    let mismatch = document(2, text, vec![mismatched_result]);
    assert!(validate(2, text, &mismatch).is_err());

    let mut invalid = calculator;
    // serde_json cannot represent infinity in canonical JSON; the evaluator tests below
    // exercise overflow produced by otherwise finite input values.
    invalid["formula_ast"]["right"]["value"] = json!(101);
    invalid["inputs"][0]["max_value"] = json!(10);
    let bad = document(2, text, vec![invalid]);
    assert!(validate(2, text, &bad).is_err());
}

#[test]
fn calc_ast_evaluator_is_deterministic_and_rejects_invalid_arithmetic() {
    let empty = std::collections::BTreeMap::new();
    let round_positive = json!({
        "kind":"FUNCTION",
        "function":"ROUND",
        "arguments":[
            {"kind":"LITERAL","value":1.25},
            {"kind":"LITERAL","value":1}
        ]
    });
    assert_eq!(calc::evaluate_calc_ast(&round_positive, &empty), Ok(1.3));

    let round_negative_tie = json!({
        "kind":"FUNCTION",
        "function":"ROUND",
        "arguments":[
            {"kind":"LITERAL","value":-1.25},
            {"kind":"LITERAL","value":1}
        ]
    });
    assert_eq!(
        calc::evaluate_calc_ast(&round_negative_tie, &empty),
        Ok(-1.3)
    );

    let round_hundreds = json!({
        "kind":"FUNCTION",
        "function":"ROUND",
        "arguments":[
            {"kind":"LITERAL","value":151},
            {"kind":"LITERAL","value":-2}
        ]
    });
    assert_eq!(calc::evaluate_calc_ast(&round_hundreds, &empty), Ok(200.0));

    let divide_by_zero = json!({
        "kind":"BINARY_OP",
        "operator":"/",
        "left":{"kind":"LITERAL","value":1},
        "right":{"kind":"LITERAL","value":0}
    });
    assert!(calc::evaluate_calc_ast(&divide_by_zero, &empty).is_err());

    let overflow = json!({
        "kind":"BINARY_OP",
        "operator":"*",
        "left":{"kind":"LITERAL","value":1e308},
        "right":{"kind":"LITERAL","value":1e308}
    });
    assert!(calc::evaluate_calc_ast(&overflow, &empty).is_err());
}

#[test]
fn calc_ast_validation_rejects_every_structural_profile_violation() {
    let declared = std::collections::BTreeSet::from(["amount".to_string()]);
    let literal = json!({"kind":"LITERAL","value":1});
    let bad_nodes = vec![
        json!({"kind":"UNKNOWN"}),
        json!({"kind":"LITERAL","value":"1"}),
        json!({"kind":"LITERAL","value":1,"extra":true}),
        json!({"kind":"VARIABLE","input_id":1}),
        json!({"kind":"VARIABLE","input_id":"missing"}),
        json!({"kind":"BINARY_OP","operator":"^","left":literal,"right":literal}),
        json!({"kind":"BINARY_OP","operator":"+","left":literal}),
        json!({"kind":"BINARY_OP","operator":"+","left":literal,"right":literal,"extra":null}),
        json!({"kind":"UNARY_OP","operator":"+","operand":literal}),
        json!({"kind":"FUNCTION","function":"POWER","arguments":[literal,literal]}),
        json!({"kind":"FUNCTION","function":"ABS","arguments":[]}),
        json!({"kind":"FUNCTION","function":"ABS","arguments":[literal,literal]}),
        json!({"kind":"FUNCTION","function":"MIN","arguments":[literal]}),
        json!({"kind":"FUNCTION","function":"MIN","arguments":vec![literal.clone(); 9]}),
        json!({"kind":"FUNCTION","function":"ROUND","arguments":[]}),
        json!({"kind":"FUNCTION","function":"ROUND","arguments":[literal,{"kind":"LITERAL","value":1.5}]}),
        json!({"kind":"FUNCTION","function":"ROUND","arguments":[literal,{"kind":"LITERAL","value":7}]}),
        json!({"kind":"FUNCTION","function":"ROUND","arguments":[literal,{"kind":"VARIABLE","input_id":"amount"}]}),
        json!({"kind":"FUNCTION","function":"CLAMP","arguments":[literal,{"kind":"LITERAL","value":5},{"kind":"LITERAL","value":2}]}),
        json!({"kind":"FUNCTION","function":"CLAMP","arguments":[literal,literal]}),
    ];

    for ast in bad_nodes {
        assert!(
            calc::validate_calc_ast(&ast, &declared).is_err(),
            "unexpectedly accepted AST: {ast}"
        );
    }
}

#[test]
fn calc_ast_complexity_limits_accept_exact_bounds_and_reject_overages() {
    fn balanced(depth: usize) -> serde_json::Value {
        if depth == 0 {
            json!({"kind":"LITERAL","value":1})
        } else {
            json!({"kind":"BINARY_OP","operator":"+","left":balanced(depth - 1),"right":balanced(depth - 1)})
        }
    }
    fn unary_chain(count: usize) -> serde_json::Value {
        (0..count).fold(
            json!({"kind":"LITERAL","value":1}),
            |operand, _| json!({"kind":"UNARY_OP","operator":"-","operand":operand}),
        )
    }

    let declared = std::collections::BTreeSet::new();
    // A complete depth-five binary tree has 63 nodes; one unary root reaches 64.
    let max_nodes = json!({"kind":"UNARY_OP","operator":"-","operand":balanced(5)});
    assert!(calc::validate_calc_ast(&max_nodes, &declared).is_ok());
    // One more root makes 65 nodes without exceeding the depth limit.
    let over_nodes = json!({"kind":"UNARY_OP","operator":"-","operand":max_nodes});
    assert!(calc::validate_calc_ast(&over_nodes, &declared).is_err());

    assert!(calc::validate_calc_ast(&unary_chain(12), &declared).is_ok());
    assert!(calc::validate_calc_ast(&unary_chain(13), &declared).is_err());
}

#[test]
fn calc_ast_function_arity_and_runtime_failures_are_bounded() {
    let empty = std::collections::BTreeMap::new();
    let declared = std::collections::BTreeSet::new();
    let min = json!({"kind":"FUNCTION","function":"MIN","arguments":[{"kind":"LITERAL","value":4},{"kind":"LITERAL","value":-2}]});
    let max_eight = json!({
        "kind":"FUNCTION",
        "function":"MAX",
        "arguments":vec![json!({"kind":"LITERAL","value":1}); 8]
    });
    assert_eq!(calc::evaluate_calc_ast(&min, &empty), Ok(-2.0));
    assert_eq!(calc::evaluate_calc_ast(&max_eight, &empty), Ok(1.0));

    for operator in ["/", "%"] {
        let divide_by_zero = json!({"kind":"BINARY_OP","operator":operator,"left":{"kind":"LITERAL","value":1},"right":{"kind":"LITERAL","value":0}});
        assert!(calc::validate_calc_ast(&divide_by_zero, &declared).is_ok());
        assert!(calc::evaluate_calc_ast(&divide_by_zero, &empty).is_err());
    }

    let missing_input = json!({"kind":"VARIABLE","input_id":"amount"});
    assert!(calc::evaluate_calc_ast(&missing_input, &empty).is_err());
}

#[test]
fn canonical_bytes_digest_and_unknown_fields_fail_closed() {
    let text = "Hello";
    let good = document(
        1,
        text,
        vec![json!({"kind":"TEXT_SLICE","source":slice(text, text)})],
    );
    let mut noncanonical = good.clone();
    noncanonical.push(b' ');
    assert!(validate(1, text, &noncanonical).is_err());
    let unknown = document(
        1,
        text,
        vec![json!({"kind":"TEXT_SLICE","source":slice(text, text),"html":"<script>"})],
    );
    assert!(validate(1, text, &unknown).is_err());
}

#[test]
fn table_cell_number_rendering_matches_ecmascript_string_conversion() {
    for (value, expected) in [
        (0.0_f64, "0"),
        (100.0, "100"),
        (0.1, "0.1"),
        (12.5, "12.5"),
        (-0.5, "-0.5"),
        (1.0 / 3.0, "0.3333333333333333"),
        (0.000_001, "0.000001"),
        (1e-7, "1e-7"),
        (5e-7, "5e-7"),
        (1e20, "100000000000000000000"),
        (1e21, "1e+21"),
        (-1e21, "-1e+21"),
        (1e22, "1e+22"),
        (9_007_199_254_740_991.0, "9007199254740991"),
    ] {
        assert_eq!(es_number_string(value).as_deref(), Some(expected));
    }
    assert_eq!(es_number_string(f64::INFINITY), None);
    assert_eq!(es_number_string(f64::NAN), None);
    assert_eq!(es_number_string(-0.0).as_deref(), Some("0"));
}

#[test]
fn table_semantic_match_preserves_existing_row_collection_behavior() {
    let headers = ["Metric", "Value"];
    let two_rows = "| Metric | Value |\n| --- | --- |\n| Cost | 12 |\n| Tiny | 1e-27 |\n";
    assert!(table_matches_semantic_markdown(
        two_rows,
        &headers,
        &[
            vec![json!("Cost"), json!(12)],
            vec![json!("Tiny"), json!(1e-27)]
        ]
    ));

    let truncated = "| Metric | Value |\n| --- | --- |\n| only-one-cell |\n";
    assert!(table_matches_semantic_markdown(truncated, &headers, &[]));
    assert!(!table_matches_semantic_markdown(
        truncated,
        &headers,
        &[vec![json!("only-one-cell"), json!("")]]
    ));

    let one_row = "| Metric | Value |\n| --- | --- |\n| Cost | 12 |\nProse after the table.\n";
    assert!(table_matches_semantic_markdown(
        one_row,
        &headers,
        &[vec![json!("Cost"), json!(12)]]
    ));
    assert!(!table_matches_semantic_markdown(
        one_row,
        &headers,
        &[
            vec![json!("Cost"), json!(12)],
            vec![json!("Cost"), json!(12)]
        ]
    ));
    assert!(!table_matches_semantic_markdown(
        "| Metric | Value |\n| --- | --- |\n| a\\|b | 12 |\n",
        &headers,
        &[vec![json!("a|b"), json!(12)]]
    ));
}

#[test]
fn unsupported_components_and_unavailable_document_capabilities_fail_closed() {
    let text = "Hello";
    for kind in [
        "HOST_PROJECTION",
        "MCP_APP",
        "DIAGRAM",
        "MEDIA",
        "CHECKLIST",
        "DELIVERABLE_GROUP",
        "ARTIFACT_COLLECTION",
        "UNREGISTERED",
    ] {
        let bytes = document(1, text, vec![json!({"kind":kind})]);
        assert_eq!(
            validate(1, text, &bytes),
            Err(ValidationError::UnsupportedBlock),
            "{kind} must not enter the renderer-v1 profile"
        );
    }
    for kind in ["METRIC", "CHART", "SAFE_CALCULATOR"] {
        let bytes = document(1, text, vec![json!({"kind":kind})]);
        assert_eq!(
            validate(1, text, &bytes),
            Err(ValidationError::UnsupportedBlock),
            "{kind} is only supported by renderer-v2"
        );
    }

    let base = document(
        1,
        text,
        vec![json!({"kind":"TEXT_SLICE","source":slice(text, text)})],
    );
    for key in ["actions", "citations"] {
        let mut value: Value = serde_json::from_slice(&base).unwrap();
        value[key] = json!([{"unresolved":"source or action binder is unavailable"}]);
        let bytes = serde_json_canonicalizer::to_vec(&value).unwrap();
        assert!(
            matches!(validate(1, text, &bytes), Err(ValidationError::Invalid(_))),
            "non-empty {key} must fail closed until its owner binder exists"
        );
    }
}

#[test]
fn exact_document_limit_is_accepted() {
    // Keep the semantic Markdown table within the independent 1 MiB semantic cap while
    // tuning one bounded cell so the canonical presentation is exactly 512 KiB.
    fn build(last_cell_len: usize) -> (String, Vec<u8>) {
        let mut values = vec!["x".repeat(1_040); 500];
        values[499] = "x".repeat(last_cell_len);
        let mut semantic = String::from("| Value |\n| --- |\n");
        for value in &values {
            semantic.push_str("| ");
            semantic.push_str(value);
            semantic.push_str(" |\n");
        }
        let table = json!({
            "kind":"TABLE",
            "headers":["Value"],
            "rows":values.iter().map(|value| json!([value])).collect::<Vec<_>>()
        });
        let bytes = document(1, &semantic, vec![table]);
        (semantic, bytes)
    }

    let mut low = 0usize;
    let mut high = 8_192usize;
    let mut exact = None;
    while low <= high {
        let middle = low + (high - low) / 2;
        let candidate = build(middle);
        match candidate.1.len().cmp(&MAX_DOCUMENT_BYTES) {
            std::cmp::Ordering::Equal => {
                exact = Some(candidate);
                break;
            }
            std::cmp::Ordering::Less => low = middle + 1,
            std::cmp::Ordering::Greater => {
                if middle == 0 {
                    break;
                }
                high = middle - 1;
            }
        }
    }
    let (semantic, bytes) =
        exact.expect("a bounded table cell can fill the document to its exact cap");
    assert_eq!(bytes.len(), MAX_DOCUMENT_BYTES);
    assert_eq!(validate(1, &semantic, &bytes), Ok(()));
}

fn corpus_table_text() -> String {
    "Revenue for Feb is reported in INR at lakh scale for the monthly interval with sum aggregation.\n\nRevenue by Month is reported in INR at lakh scale for the monthly interval with sum aggregation.\n\nRevenue trend.\n\n| Month | Revenue |\n| --- | --- |\n| Jan | 12 |\n| Feb | 18 |".to_owned()
}

fn corpus_metric_chart_blocks(text: &str, bad_index: bool, context_mode: &str) -> Vec<Value> {
    let table = "| Month | Revenue |\n| --- | --- |\n| Jan | 12 |\n| Feb | 18 |";
    let mut metric = json!({
        "kind":"METRIC",
        "binding":{
            "source":"SEMANTIC_TABLE",
            "table_slice":slice(text, table),
            "row_index":if bad_index { 99 } else { 1 },
            "column_index":1
        }
    });
    let mut chart = json!({
        "kind":"CHART",
        "chart_kind":"LINE",
        "binding":{"source":"SEMANTIC_TABLE","semantic_table_slice":slice(text, table)},
        "category_column_index":0,
        "series_column_indexes":[1],
        "accessible_summary":if context_mode == "ungrounded" { "Revenue trend." } else { "Revenue by Month is reported in INR at lakh scale for the monthly interval with sum aggregation." }
    });
    if context_mode != "missing" {
        let semantics = match context_mode {
            "unsafe" => {
                json!({"unit":"INR\u{202e}","scale":"LAKH","time_interval":"monthly","aggregation":"SUM"})
            }
            "invalid-enum" => {
                json!({"unit":"INR","scale":"UNSUPPORTED","time_interval":"monthly","aggregation":"SUM"})
            }
            "unit-substring" => {
                json!({"unit":"IN","scale":"LAKH","time_interval":"monthly","aggregation":"SUM"})
            }
            _ => json!({"unit":"INR","scale":"LAKH","time_interval":"monthly","aggregation":"SUM"}),
        };
        metric
            .as_object_mut()
            .unwrap()
            .insert("semantics".to_owned(), semantics.clone());
        metric.as_object_mut().unwrap().insert(
            "accessible_summary".to_owned(),
            json!(if context_mode == "ungrounded" { "Revenue trend." } else { "Revenue for Feb is reported in INR at lakh scale for the monthly interval with sum aggregation." }),
        );
        chart
            .as_object_mut()
            .unwrap()
            .insert("semantics".to_owned(), semantics);
    }
    vec![metric, chart]
}

fn corpus_calculator(text: &str, bad_result: bool) -> Value {
    let input_table = "| Input | Value |\n| --- | --- |\n| Amount | 12 |";
    let result_table =
        "| Input | Value |\n| --- | --- |\n| Amount | 12 |\n| Estimated amount | 24 |";
    json!({
        "kind":"SAFE_CALCULATOR",
        "inputs":[{
            "id":"amount",
            "label":"Amount",
            "binding":{
                "source":"SEMANTIC_TABLE",
                "table_slice":slice(text, input_table),
                "row_index":0,
                "column_index":1
            },
            "min_value":0,
            "max_value":100,
            "step":1
        }],
        "formula_ast":{"kind":"BINARY_OP","operator":"*","left":{"kind":"VARIABLE","input_id":"amount"},"right":{"kind":"LITERAL","value":2}},
        "result_binding":{
            "source":"SEMANTIC_TABLE",
            "table_slice":slice(text, result_table),
            "row_index":if bad_result { 0 } else { 1 },
            "column_index":1
        },
        "output_format":{"style":"NUMBER"},
        "fallback_text":"The estimated amount is shown below."
    })
}

fn corpus_boundary_document(last_cell_length: usize) -> (String, Vec<u8>) {
    let mut cells = vec!["x".repeat(1_040); 500];
    cells[499] = "x".repeat(last_cell_length);
    let mut semantic = String::from("| Value |\n| --- |\n");
    for cell in &cells {
        semantic.push_str("| ");
        semantic.push_str(cell);
        semantic.push_str(" |\n");
    }
    let block = json!({
        "kind":"TABLE",
        "headers":["Value"],
        "rows":cells.iter().map(|cell| json!([cell])).collect::<Vec<_>>()
    });
    let bytes = document(1, &semantic, vec![block]);
    (semantic, bytes)
}

fn exact_corpus_boundary_document() -> (String, Vec<u8>) {
    let mut low = 0usize;
    let mut high = 8_192usize;
    while low <= high {
        let middle = low + (high - low) / 2;
        let candidate = corpus_boundary_document(middle);
        match candidate.1.len().cmp(&MAX_DOCUMENT_BYTES) {
            std::cmp::Ordering::Equal => return candidate,
            std::cmp::Ordering::Less => low = middle + 1,
            std::cmp::Ordering::Greater => high = middle - 1,
        }
    }
    panic!("corpus fixture must be tunable to the exact document-byte boundary")
}

fn corpus_case_result(scenario: &str) -> Result<(), ValidationError> {
    if scenario == "NON_FINITE_JSON_REJECTED" {
        let bytes = br#"{"schema_version":1,"renderer_contract_version":1,"presentation_id":"presentation-test","message_id":"message-test","semantic_content_digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","root_blocks":[{"kind":"TABLE","headers":["Value"],"rows":[[1e400]]}],"block_provenance":[{"block_path":"/0","origin":"SEMANTIC_MESSAGE","resource_refs":[],"artifact_refs":[],"evidence_refs":[],"verification_refs":[]}]}"#;
        return validate(1, "| Value |\n| --- |\n| 1 |", bytes);
    }
    if matches!(
        scenario,
        "MAX_SAFE_INTEGER_VALID"
            | "UNSAFE_INTEGER_TOKEN_REJECTED"
            | "UNSAFE_EXPONENT_INTEGER_REJECTED"
    ) {
        let token = match scenario {
            "MAX_SAFE_INTEGER_VALID" => "9007199254740991",
            "UNSAFE_INTEGER_TOKEN_REJECTED" => "9007199254740992",
            _ => "1e+21",
        };
        let semantic = format!("| Value |\n| --- |\n| {token} |");
        let bytes = format!(
            "{{\"block_provenance\":[{{\"artifact_refs\":[],\"block_path\":\"/0\",\"evidence_refs\":[],\"origin\":\"SEMANTIC_MESSAGE\",\"resource_refs\":[],\"verification_refs\":[]}}],\"message_id\":\"message-test\",\"presentation_id\":\"presentation-test\",\"renderer_contract_version\":1,\"root_blocks\":[{{\"headers\":[\"Value\"],\"kind\":\"TABLE\",\"rows\":[[{token}]]}}],\"schema_version\":1,\"semantic_content_digest\":\"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}}"
        );
        return validate(1, &semantic, bytes.as_bytes());
    }
    let (version, text, blocks) = match scenario {
        "V1_TABLE_VALID" => {
            let text = "Summary\n\n| Metric | Value |\n| --- | --- |\n| Total | 12 |";
            (
                1,
                text.to_owned(),
                vec![json!({"kind":"TABLE","headers":["Metric","Value"],"rows":[["Total",12]]})],
            )
        }
        "V2_METRIC_CHART_VALID" => {
            let text = corpus_table_text();
            let blocks = corpus_metric_chart_blocks(&text, false, "valid");
            (2, text, blocks)
        }
        "V2_METRIC_CHART_MISSING_CONTEXT" => {
            let text = corpus_table_text();
            let blocks = corpus_metric_chart_blocks(&text, false, "missing");
            (2, text, blocks)
        }
        "V2_METRIC_CHART_UNGROUNDED_CONTEXT" => {
            let text = corpus_table_text();
            let blocks = corpus_metric_chart_blocks(&text, false, "ungrounded");
            (2, text, blocks)
        }
        "V2_METRIC_CHART_UNSAFE_CONTEXT" => {
            let text = corpus_table_text();
            let blocks = corpus_metric_chart_blocks(&text, false, "unsafe");
            (2, text, blocks)
        }
        "V2_METRIC_CHART_INVALID_ENUM" => {
            let text = corpus_table_text();
            let blocks = corpus_metric_chart_blocks(&text, false, "invalid-enum");
            (2, text, blocks)
        }
        "V2_METRIC_CHART_TOKEN_SUBSTRING" => {
            let text = corpus_table_text();
            let blocks = corpus_metric_chart_blocks(&text, false, "unit-substring");
            (2, text, blocks)
        }
        "V2_CALCULATOR_VALID" | "V2_CALCULATOR_BAD_RESULT" => {
            let text = "Estimate.\n\n| Input | Value |\n| --- | --- |\n| Amount | 12 |\n| Estimated amount | 24 |\nThe estimated amount is shown below.";
            let block = corpus_calculator(text, scenario == "V2_CALCULATOR_BAD_RESULT");
            (2, text.to_owned(), vec![block])
        }
        "V1_HOSTILE_TEXT_VALID" => {
            let text = "<img src=x onerror=alert(1)> [link](javascript:alert(1))";
            (
                1,
                text.to_owned(),
                vec![json!({"kind":"TEXT_SLICE","source":slice(text, text)})],
            )
        }
        "V1_UNKNOWN_KIND" => (
            1,
            "Summary".to_owned(),
            vec![json!({"kind":"FUTURE_WIDGET","payload":{"run":"anything"}})],
        ),
        "V1_REJECTED_KINDS" => {
            let kinds = [
                "HOST_PROJECTION",
                "MCP_APP",
                "DIAGRAM",
                "MEDIA",
                "CHECKLIST",
                "DELIVERABLE_GROUP",
                "METRIC",
                "CHART",
                "SAFE_CALCULATOR",
            ];
            (
                1,
                "Summary".to_owned(),
                kinds.iter().map(|kind| json!({"kind":kind})).collect(),
            )
        }
        "V2_REJECTED_KINDS" => {
            let kinds = [
                "HOST_PROJECTION",
                "MCP_APP",
                "DIAGRAM",
                "MEDIA",
                "CHECKLIST",
                "DELIVERABLE_GROUP",
                "INTERACTIVE_ARTIFACT",
                "FUTURE_WIDGET",
            ];
            (
                2,
                "Summary".to_owned(),
                kinds.iter().map(|kind| json!({"kind":kind})).collect(),
            )
        }
        "V2_METRIC_BAD_INDEX" => {
            let text = corpus_table_text();
            (
                2,
                text.clone(),
                corpus_metric_chart_blocks(&text, true, "valid"),
            )
        }
        "MAX_NESTING_VALID" | "EXCESS_NESTING_REJECTED" => {
            let text = "Summary";
            let count = if scenario == "MAX_NESTING_VALID" {
                9
            } else {
                10
            };
            let mut nested = json!({"kind":"CARD","title":"Summary","children":[]});
            for _ in 1..count {
                nested = json!({"kind":"CARD","title":"Summary","children":[nested]});
            }
            (1, text.to_owned(), vec![nested])
        }
        "MAX_BLOCK_COUNT_VALID" | "EXCESS_BLOCK_COUNT_REJECTED" => {
            let count = if scenario == "MAX_BLOCK_COUNT_VALID" {
                200
            } else {
                201
            };
            (
                1,
                "Summary".to_owned(),
                (0..count)
                    .map(|_| json!({"kind":"CARD","title":"Summary","children":[]}))
                    .collect(),
            )
        }
        "EXCESS_DOCUMENT_BYTES_REJECTED" => {
            let (text, exact) = exact_corpus_boundary_document();
            let mut value: Value = serde_json::from_slice(&exact).unwrap();
            let rows = value["root_blocks"][0]["rows"].as_array_mut().unwrap();
            let new_length = rows[499][0].as_str().unwrap().len() + 1;
            rows[499][0] = json!("x".repeat(new_length));
            let bytes = serde_json_canonicalizer::to_vec(&value).unwrap();
            return validate(1, &text, &bytes);
        }
        "MAX_DOCUMENT_BYTES_VALID" => {
            let (text, bytes) = exact_corpus_boundary_document();
            return validate(1, &text, &bytes);
        }
        _ => panic!("unknown corpus scenario {scenario}"),
    };
    let bytes = document(version, &text, blocks);
    validate(version, &text, &bytes)
}

#[test]
fn shared_golden_corpus_matches_expected_acceptance() {
    let corpus: Value = serde_json::from_str(include_str!("../tests/corpus/cases.json")).unwrap();
    assert_eq!(corpus["corpus_version"], 1);
    let cases = corpus["cases"].as_array().unwrap();
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let scenario = case["scenario"].as_str().unwrap();
        let expected = case["expected"]["accept"].as_bool().unwrap();
        assert_eq!(
            case["expected"]["reason"].as_str().is_some(),
            true,
            "{id} needs a reason"
        );
        let result = corpus_case_result(scenario);
        assert_eq!(
            result.is_ok(),
            expected,
            "shared corpus case {id}: {scenario}; result={result:?}"
        );
    }
}

#[test]
fn non_finite_json_number_fails_closed_in_shared_corpus() {
    assert!(corpus_case_result("NON_FINITE_JSON_REJECTED").is_err());
}

#[test]
fn unsafe_integer_valued_json_numbers_fail_closed_in_shared_corpus() {
    assert!(corpus_case_result("MAX_SAFE_INTEGER_VALID").is_ok());
    assert!(corpus_case_result("UNSAFE_INTEGER_TOKEN_REJECTED").is_err());
    assert!(corpus_case_result("UNSAFE_EXPONENT_INTEGER_REJECTED").is_err());
}
