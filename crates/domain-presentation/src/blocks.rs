use crate::{ValidationError, calc, digest};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn closed_object(
    value: &Value,
    required: &[&str],
    optional: &[&str],
) -> Result<(), ValidationError> {
    let object = value
        .as_object()
        .ok_or(ValidationError::Invalid("presentation object required"))?;
    if required.iter().any(|key| !object.contains_key(*key))
        || object
            .keys()
            .any(|key| !required.contains(&key.as_str()) && !optional.contains(&key.as_str()))
    {
        return Err(ValidationError::Invalid(
            "presentation object has missing/unknown fields",
        ));
    }
    Ok(())
}

pub(crate) fn within_code_units(value: &str, max: usize) -> bool {
    value.encode_utf16().count() <= max && !value.contains('\0')
}

pub(crate) fn semantic_line_matches(semantic_text: &str, candidate: &str) -> bool {
    semantic_text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .any(|line| {
            let line = strip_heading(line.trim());
            strip_emphasis(line).unwrap_or(line) == candidate
        })
}

fn strip_heading(line: &str) -> &str {
    let hashes = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    match line.as_bytes().get(hashes) {
        Some(b' ' | b'\t') if (1..=6).contains(&hashes) => line[hashes..].trim(),
        _ => line,
    }
}

fn strip_emphasis(line: &str) -> Option<&str> {
    for delimiter in ["**", "__", "*", "_"] {
        if let Some(rest) = line.strip_prefix(delimiter)
            && let Some(inner) = rest.strip_suffix(delimiter)
            && !inner.trim().is_empty()
        {
            return Some(inner.trim());
        }
    }
    None
}

pub(crate) fn validate_block(
    block: &Value,
    path: &str,
    semantic_text: &str,
    renderer_version: u32,
    depth: usize,
    paths: &mut Vec<String>,
) -> Result<(), ValidationError> {
    if depth > 8 || paths.len() >= 200 {
        return Err(ValidationError::Invalid(
            "presentation nesting/block limit exceeded",
        ));
    }
    paths.push(path.to_owned());
    if let Some(width) = block.get("width")
        && !matches!(width.as_str(), Some("READABLE" | "WIDE" | "FULL_AVAILABLE"))
    {
        return Err(ValidationError::Invalid("invalid width"));
    }

    match block["kind"].as_str() {
        Some("TEXT_SLICE") => {
            closed_object(block, &["kind", "source"], &["width"])?;
            validate_text_source(&block["source"], semantic_text)?;
        }
        Some("LAYOUT") => {
            closed_object(block, &["kind", "layout", "children"], &["width"])?;
            if !matches!(block["layout"].as_str(), Some("STACK" | "ROW" | "GRID")) {
                return Err(ValidationError::Invalid("invalid layout"));
            }
            let children = block["children"]
                .as_array()
                .ok_or(ValidationError::Invalid("children array required"))?;
            if children.is_empty() || children.len() > 20 {
                return Err(ValidationError::Invalid("invalid layout child count"));
            }
            for (index, child) in children.iter().enumerate() {
                validate_block(
                    child,
                    &format!("{path}/children/{index}"),
                    semantic_text,
                    renderer_version,
                    depth + 1,
                    paths,
                )?;
            }
        }
        Some("CARD") => {
            closed_object(block, &["kind", "title", "children"], &[])?;
            let title = block["title"]
                .as_str()
                .ok_or(ValidationError::Invalid("invalid card title"))?;
            if !within_code_units(title, 300)
                || title.trim().is_empty()
                || !semantic_line_matches(semantic_text, title)
            {
                return Err(ValidationError::Invalid(
                    "card title must repeat a semantic line",
                ));
            }
            let children = block["children"]
                .as_array()
                .ok_or(ValidationError::Invalid("children array required"))?;
            if children.len() > 20 {
                return Err(ValidationError::Invalid("invalid card child count"));
            }
            for (index, child) in children.iter().enumerate() {
                validate_block(
                    child,
                    &format!("{path}/children/{index}"),
                    semantic_text,
                    renderer_version,
                    depth + 1,
                    paths,
                )?;
            }
        }
        Some("CALLOUT") => {
            closed_object(
                block,
                &["kind", "tone", "source", "accessible_summary"],
                &[],
            )?;
            if !matches!(block["tone"].as_str(), Some("NEUTRAL" | "INFO" | "WARNING")) {
                return Err(ValidationError::Invalid("invalid callout tone"));
            }
            let summary = block["accessible_summary"]
                .as_str()
                .ok_or(ValidationError::Invalid("invalid callout summary"))?;
            if !within_code_units(summary, 1_000) || !semantic_line_matches(semantic_text, summary)
            {
                return Err(ValidationError::Invalid(
                    "callout summary must repeat a semantic line",
                ));
            }
            validate_text_source(&block["source"], semantic_text)?;
        }
        Some("TABLE") => validate_table(block, semantic_text)?,
        Some("METRIC") if renderer_version == 2 => {
            validate_metric(block, semantic_text)?;
        }
        Some("CHART") if renderer_version == 2 => {
            validate_chart(block, semantic_text)?;
        }
        Some("SAFE_CALCULATOR") if renderer_version == 2 => {
            validate_calculator(block, semantic_text)?;
        }
        Some("METRIC" | "CHART" | "SAFE_CALCULATOR") => {
            return Err(ValidationError::UnsupportedBlock);
        }
        _ => return Err(ValidationError::UnsupportedBlock),
    }
    Ok(())
}

fn validate_text_source(source: &Value, semantic_text: &str) -> Result<(), ValidationError> {
    closed_object(
        source,
        &["start_utf8_byte", "end_utf8_byte_exclusive", "slice_digest"],
        &[],
    )?;
    let (start, end) = slice_bounds(source, "start_utf8_byte", "end_utf8_byte_exclusive")?;
    let bytes = semantic_text.as_bytes();
    if start >= end || end > bytes.len() {
        return Err(ValidationError::Invalid("text slice outside semantic text"));
    }
    let slice = std::str::from_utf8(&bytes[start..end])
        .map_err(|_| ValidationError::Invalid("text slice is not on UTF-8 boundaries"))?;
    if source["slice_digest"] != digest(slice.as_bytes()) {
        return Err(ValidationError::Integrity(
            "semantic text slice digest mismatch",
        ));
    }
    Ok(())
}

fn slice_bounds(
    value: &Value,
    start_key: &str,
    end_key: &str,
) -> Result<(usize, usize), ValidationError> {
    let start = value[start_key]
        .as_u64()
        .and_then(|number| usize::try_from(number).ok())
        .ok_or(ValidationError::Invalid("invalid semantic slice start"))?;
    let end = value[end_key]
        .as_u64()
        .and_then(|number| usize::try_from(number).ok())
        .ok_or(ValidationError::Invalid("invalid semantic slice end"))?;
    Ok((start, end))
}

fn validate_table(block: &Value, semantic_text: &str) -> Result<(), ValidationError> {
    closed_object(block, &["kind", "headers", "rows"], &["accessible_summary"])?;
    let headers = block["headers"]
        .as_array()
        .ok_or(ValidationError::Invalid("headers array required"))?;
    if headers.is_empty() || headers.len() > 20 {
        return Err(ValidationError::Invalid("invalid table header count"));
    }
    for header in headers {
        if !header
            .as_str()
            .is_some_and(|value| within_code_units(value, 300))
        {
            return Err(ValidationError::Invalid("invalid table header"));
        }
    }
    validate_optional_summary(block, semantic_text, "accessible_summary", 2_000)?;
    let rows = block["rows"]
        .as_array()
        .ok_or(ValidationError::Invalid("rows array required"))?;
    if rows.len() > 500 {
        return Err(ValidationError::Invalid("invalid table row count"));
    }
    let mut validated = Vec::with_capacity(rows.len());
    for row in rows {
        let row = row
            .as_array()
            .ok_or(ValidationError::Invalid("invalid table row"))?;
        if row.len() != headers.len() {
            return Err(ValidationError::Invalid("table row width mismatch"));
        }
        for cell in row {
            let valid = match cell {
                Value::Null | Value::Bool(_) => true,
                Value::Number(number) => number.as_f64().is_some_and(f64::is_finite),
                Value::String(value) => within_code_units(value, 8_192),
                Value::Array(_) | Value::Object(_) => false,
            };
            if !valid {
                return Err(ValidationError::Invalid("invalid table cell"));
            }
        }
        validated.push(row.clone());
    }
    let headers: Vec<&str> = headers
        .iter()
        .map(|value| value.as_str().unwrap_or_default())
        .collect();
    if !table_matches_semantic_markdown(semantic_text, &headers, &validated) {
        return Err(ValidationError::Invalid(
            "table must repeat a semantic Markdown table exactly",
        ));
    }
    Ok(())
}

fn validate_optional_summary(
    block: &Value,
    semantic_text: &str,
    key: &str,
    max: usize,
) -> Result<(), ValidationError> {
    if let Some(value) = block.get(key) {
        let summary = value
            .as_str()
            .ok_or(ValidationError::Invalid("invalid presentation summary"))?;
        if !within_code_units(summary, max) || !semantic_line_matches(semantic_text, summary) {
            return Err(ValidationError::Invalid(
                "presentation summary must repeat a semantic line",
            ));
        }
    }
    Ok(())
}

fn validate_metric(block: &Value, semantic_text: &str) -> Result<(), ValidationError> {
    closed_object(
        block,
        &["kind", "binding", "semantics", "accessible_summary"],
        &[],
    )?;
    let summary = block["accessible_summary"]
        .as_str()
        .ok_or(ValidationError::Invalid(
            "metric accessible summary required",
        ))?;
    if !safe_context_text(summary, 2_000) || !semantic_line_matches(semantic_text, summary) {
        return Err(ValidationError::Invalid(
            "metric summary must repeat a safe semantic line",
        ));
    }
    let binding = bound_table(block.get("binding").unwrap(), semantic_text, "table_slice")?;
    let table = parse_exact_table(binding)?;
    let row_index = index(&block["binding"]["row_index"], table.rows.len())?;
    let column_index = index(&block["binding"]["column_index"], table.headers.len())?;
    let value = table.rows[row_index][column_index].trim();
    parse_finite_number(value).ok_or(ValidationError::Invalid(
        "metric must bind one finite numeric table cell",
    ))?;
    let mut measure_labels = vec![table.headers[column_index].trim()];
    if table.headers.len() > 1 {
        measure_labels.push(table.rows[row_index][0].trim());
    }
    validate_metric_semantics(&block["semantics"], summary, &measure_labels)?;
    Ok(())
}

fn validate_chart(block: &Value, semantic_text: &str) -> Result<(), ValidationError> {
    closed_object(
        block,
        &[
            "kind",
            "chart_kind",
            "binding",
            "category_column_index",
            "series_column_indexes",
            "accessible_summary",
            "semantics",
        ],
        &[],
    )?;
    if !matches!(block["chart_kind"].as_str(), Some("LINE" | "BAR" | "AREA")) {
        return Err(ValidationError::Invalid("unsupported chart kind"));
    }
    let summary = block["accessible_summary"]
        .as_str()
        .ok_or(ValidationError::Invalid(
            "chart accessible summary required",
        ))?;
    if !safe_context_text(summary, 2_000) || !semantic_line_matches(semantic_text, summary) {
        return Err(ValidationError::Invalid(
            "chart summary must repeat a semantic line",
        ));
    }
    let binding = block.get("binding").unwrap();
    let table_text = bound_table_slice(binding, semantic_text, "semantic_table_slice")?;
    let table = parse_exact_table(table_text)?;
    if table.rows.len() < 2 || table.rows.len() > 60 {
        return Err(ValidationError::Invalid(
            "chart row count must be between 2 and 60",
        ));
    }
    let category = index(&block["category_column_index"], table.headers.len())?;
    let series = block["series_column_indexes"]
        .as_array()
        .ok_or(ValidationError::Invalid("chart series indexes required"))?;
    if series.is_empty() || series.len() > 6 {
        return Err(ValidationError::Invalid("chart needs 1 to 6 series"));
    }
    let mut columns = BTreeSet::new();
    columns.insert(category);
    let mut measure_labels = vec![table.headers[category].trim()];
    for value in series {
        let column = index(value, table.headers.len())?;
        if !columns.insert(column) {
            return Err(ValidationError::Invalid(
                "chart category and series columns must be distinct",
            ));
        }
        measure_labels.push(table.headers[column].trim());
    }
    for row in &table.rows {
        if row[category].trim().is_empty() {
            return Err(ValidationError::Invalid("chart categories cannot be empty"));
        }
        for column in series {
            let column = index(column, table.headers.len())?;
            parse_finite_number(row[column].trim()).ok_or(ValidationError::Invalid(
                "chart series cells must be finite numbers",
            ))?;
        }
    }
    validate_metric_semantics(&block["semantics"], summary, &measure_labels)?;
    Ok(())
}

fn safe_context_text(value: &str, max_chars: usize) -> bool {
    value.chars().count() <= max_chars
        && !value.contains('\0')
        && !value.trim().is_empty()
        && value == value.trim()
        && !value.chars().any(|character| {
            character.is_control()
                || matches!(character, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
        })
}

fn validate_metric_semantics(
    value: &Value,
    summary: &str,
    source_labels: &[&str],
) -> Result<(), ValidationError> {
    closed_object(
        value,
        &["unit", "scale", "time_interval", "aggregation"],
        &[],
    )?;
    let unit = value["unit"]
        .as_str()
        .ok_or(ValidationError::Invalid("metric unit context required"))?;
    let interval = value["time_interval"]
        .as_str()
        .ok_or(ValidationError::Invalid("metric interval context required"))?;
    let scale = match value["scale"].as_str() {
        Some("UNIT") => "unit scale",
        Some("THOUSAND") => "thousand scale",
        Some("MILLION") => "million scale",
        Some("BILLION") => "billion scale",
        Some("LAKH") => "lakh scale",
        Some("CRORE") => "crore scale",
        _ => return Err(ValidationError::Invalid("invalid metric scale context")),
    };
    let aggregation = match value["aggregation"].as_str() {
        Some("NONE") => "no aggregation",
        Some("SUM") => "sum aggregation",
        Some("AVERAGE") => "average aggregation",
        Some("COUNT") => "count aggregation",
        Some("MINIMUM") => "minimum aggregation",
        Some("MAXIMUM") => "maximum aggregation",
        Some("LAST") => "last-value aggregation",
        _ => {
            return Err(ValidationError::Invalid(
                "invalid metric aggregation context",
            ));
        }
    };
    if !safe_context_text(unit, 80) || !safe_context_text(interval, 80) {
        return Err(ValidationError::Invalid(
            "metric unit and interval context must be safe bounded labels",
        ));
    }
    if !contains_semantic_label(summary, unit)
        || !contains_semantic_label(summary, interval)
        || !contains_ascii_case_insensitive(summary, scale)
        || !contains_ascii_case_insensitive(summary, aggregation)
        || source_labels
            .iter()
            .any(|label| !contains_semantic_label(summary, label))
    {
        return Err(ValidationError::Invalid(
            "metric context and source labels must occur in the semantic summary",
        ));
    }
    Ok(())
}

fn contains_ascii_case_insensitive(value: &str, needle: &str) -> bool {
    value
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

fn contains_semantic_label(value: &str, label: &str) -> bool {
    if label.is_empty() {
        return false;
    }
    let first_is_word = label.chars().next().is_some_and(char::is_alphanumeric);
    let last_is_word = label.chars().next_back().is_some_and(char::is_alphanumeric);
    value.match_indices(label).any(|(start, matched)| {
        let before = value[..start].chars().next_back();
        let after = value[start + matched.len()..].chars().next();
        (!first_is_word || !before.is_some_and(char::is_alphanumeric))
            && (!last_is_word || !after.is_some_and(char::is_alphanumeric))
    })
}

fn validate_calculator(block: &Value, semantic_text: &str) -> Result<(), ValidationError> {
    closed_object(
        block,
        &[
            "kind",
            "inputs",
            "formula_ast",
            "result_binding",
            "output_format",
            "fallback_text",
        ],
        &[],
    )?;
    let inputs = block["inputs"]
        .as_array()
        .ok_or(ValidationError::Invalid("calculator inputs array required"))?;
    if inputs.is_empty() || inputs.len() > 8 {
        return Err(ValidationError::Invalid(
            "calculator input count must be 1 to 8",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut initial_values = BTreeMap::new();
    for input in inputs {
        closed_object(
            input,
            &["id", "label", "binding", "min_value", "max_value", "step"],
            &[],
        )?;
        let id = input["id"]
            .as_str()
            .filter(|value| valid_identifier(value))
            .ok_or(ValidationError::Invalid("invalid calculator input id"))?;
        if !ids.insert(id.to_owned()) {
            return Err(ValidationError::Invalid("duplicate calculator input id"));
        }
        let label = input["label"]
            .as_str()
            .ok_or(ValidationError::Invalid("invalid calculator input label"))?;
        if !within_code_units(label, 200) || !source_label_matches(semantic_text, label) {
            return Err(ValidationError::Invalid(
                "calculator label must repeat a semantic table label or line",
            ));
        }
        let initial = bound_finite_cell(input.get("binding").unwrap(), semantic_text)?;
        let minimum = finite_number(&input["min_value"])?;
        let maximum = finite_number(&input["max_value"])?;
        let step = finite_number(&input["step"])?;
        if minimum > maximum || step <= 0.0 || initial < minimum || initial > maximum {
            return Err(ValidationError::Invalid(
                "calculator range/step does not contain its initial value",
            ));
        }
        if !step_aligned(initial, minimum, step) {
            return Err(ValidationError::Invalid(
                "calculator initial value is not aligned to its step",
            ));
        }
        initial_values.insert(id.to_owned(), initial);
    }
    validate_output_format(&block["output_format"], semantic_text)?;
    let fallback = block["fallback_text"]
        .as_str()
        .ok_or(ValidationError::Invalid(
            "calculator fallback text required",
        ))?;
    if !within_code_units(fallback, 2_000) || !semantic_line_matches(semantic_text, fallback) {
        return Err(ValidationError::Invalid(
            "calculator fallback must repeat a semantic line",
        ));
    }
    calc::validate_calc_ast(&block["formula_ast"], &ids)?;
    let calculated = calc::evaluate_calc_ast(&block["formula_ast"], &initial_values)?;
    let expected = bound_finite_cell(&block["result_binding"], semantic_text)?;
    if !numeric_results_match(calculated, expected) {
        return Err(ValidationError::Invalid(
            "calculator initial result differs from its semantic result binding",
        ));
    }
    Ok(())
}

fn bound_finite_cell(binding: &Value, semantic_text: &str) -> Result<f64, ValidationError> {
    let table_text = bound_table(binding, semantic_text, "table_slice")?;
    let table = parse_exact_table(table_text)?;
    let row_index = index(&binding["row_index"], table.rows.len())?;
    let column_index = index(&binding["column_index"], table.headers.len())?;
    parse_finite_number(table.rows[row_index][column_index].trim()).ok_or(ValidationError::Invalid(
        "calculator binding must name a finite numeric table cell",
    ))
}

fn numeric_results_match(calculated: f64, expected: f64) -> bool {
    calculated.is_finite()
        && expected.is_finite()
        && (calculated - expected).abs() <= 1e-9 * calculated.abs().max(expected.abs()).max(1.0)
}

fn validate_output_format(value: &Value, semantic_text: &str) -> Result<(), ValidationError> {
    closed_object(value, &["style"], &["currency_code"])?;
    match value["style"].as_str() {
        Some("NUMBER" | "INTEGER") if value.get("currency_code").is_none() => Ok(()),
        Some("PERCENTAGE") if value.get("currency_code").is_none() => {
            let text = semantic_text.to_ascii_lowercase();
            if text.contains('%') || text.contains("percent") {
                Ok(())
            } else {
                Err(ValidationError::Invalid(
                    "percentage output format is not stated in semantic text",
                ))
            }
        }
        Some("CURRENCY") => {
            let code = value["currency_code"]
                .as_str()
                .ok_or(ValidationError::Invalid(
                    "currency output requires a currency code",
                ))?;
            if code.len() == 3
                && code.bytes().all(|byte| byte.is_ascii_uppercase())
                && semantic_text.contains(code)
            {
                Ok(())
            } else {
                Err(ValidationError::Invalid(
                    "currency output code must be stated in semantic text",
                ))
            }
        }
        _ => Err(ValidationError::Invalid("invalid calculator output format")),
    }
}

fn valid_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(b'A'..=b'Z' | b'a'..=b'z'))
        && bytes.all(|byte| matches!(byte, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-'))
        && value.len() <= 64
}

fn source_label_matches(semantic_text: &str, label: &str) -> bool {
    semantic_line_matches(semantic_text, label)
        || semantic_text.lines().any(|line| {
            markdown_pipe_row(line).is_some_and(|cells| cells.iter().any(|cell| *cell == label))
        })
}

fn finite_number(value: &Value) -> Result<f64, ValidationError> {
    value
        .as_f64()
        .filter(|number| number.is_finite())
        .ok_or(ValidationError::Invalid("calculator value must be finite"))
}

fn parse_finite_number(value: &str) -> Option<f64> {
    let number = value.trim().parse::<f64>().ok()?;
    number.is_finite().then_some(number)
}

fn step_aligned(value: f64, minimum: f64, step: f64) -> bool {
    let quotient = (value - minimum) / step;
    if !quotient.is_finite() {
        return false;
    }
    let nearest = quotient.round();
    (quotient - nearest).abs() <= 1e-9 * quotient.abs().max(1.0)
}

fn bound_table<'a>(
    binding: &'a Value,
    semantic_text: &'a str,
    slice_key: &str,
) -> Result<&'a str, ValidationError> {
    closed_object(
        binding,
        &["source", slice_key, "row_index", "column_index"],
        &[],
    )?;
    verify_table_slice(binding, semantic_text, slice_key)
}

fn bound_table_slice<'a>(
    binding: &'a Value,
    semantic_text: &'a str,
    slice_key: &str,
) -> Result<&'a str, ValidationError> {
    closed_object(binding, &["source", slice_key], &[])?;
    verify_table_slice(binding, semantic_text, slice_key)
}

fn verify_table_slice<'a>(
    binding: &'a Value,
    semantic_text: &'a str,
    slice_key: &str,
) -> Result<&'a str, ValidationError> {
    if binding["source"] != "SEMANTIC_TABLE" {
        return Err(ValidationError::Invalid(
            "table binding source is not semantic",
        ));
    }
    let slice = &binding[slice_key];
    closed_object(
        slice,
        &["start_utf8_byte", "end_utf8_byte_exclusive", "slice_digest"],
        &[],
    )?;
    let (start, end) = slice_bounds(slice, "start_utf8_byte", "end_utf8_byte_exclusive")?;
    let bytes = semantic_text.as_bytes();
    if start >= end || end > bytes.len() {
        return Err(ValidationError::Invalid(
            "table slice outside semantic text",
        ));
    }
    let table = std::str::from_utf8(&bytes[start..end])
        .map_err(|_| ValidationError::Invalid("table slice is not on UTF-8 boundaries"))?;
    if slice["slice_digest"] != digest(table.as_bytes()) {
        return Err(ValidationError::Integrity(
            "semantic table slice digest mismatch",
        ));
    }
    Ok(table)
}

fn index(value: &Value, length: usize) -> Result<usize, ValidationError> {
    let value = value
        .as_u64()
        .and_then(|number| usize::try_from(number).ok())
        .ok_or(ValidationError::Invalid(
            "table index must be a nonnegative integer",
        ))?;
    if value >= length {
        return Err(ValidationError::Invalid("table index is out of range"));
    }
    Ok(value)
}

struct ParsedTable<'a> {
    headers: Vec<&'a str>,
    rows: Vec<Vec<&'a str>>,
}

pub(crate) fn extract_semantic_markdown_tables(
    semantic_text: &str,
) -> Vec<crate::SemanticMarkdownTable> {
    if semantic_text.len() > crate::MAX_SEMANTIC_BYTES {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut start = 0usize;
    for with_ending in semantic_text.split_inclusive('\n') {
        let mut content_end = start + with_ending.len();
        if with_ending.ends_with('\n') {
            content_end -= 1;
            if content_end > start && semantic_text.as_bytes()[content_end - 1] == b'\r' {
                content_end -= 1;
            }
        }
        lines.push((start, content_end, start + with_ending.len()));
        start += with_ending.len();
    }

    let mut tables = Vec::new();
    let mut index = 0usize;
    while index + 2 < lines.len() && tables.len() < 200 {
        let header_line = &semantic_text[lines[index].0..lines[index].1];
        let separator_line = &semantic_text[lines[index + 1].0..lines[index + 1].1];
        let Some(headers) = markdown_pipe_row(header_line) else {
            index += 1;
            continue;
        };
        let Some(separator) = markdown_pipe_row(separator_line) else {
            index += 1;
            continue;
        };
        if headers.is_empty()
            || headers.len() > 20
            || separator.len() != headers.len()
            || !separator.iter().all(|cell| is_markdown_separator(cell))
        {
            index += 1;
            continue;
        }

        let mut rows: Vec<Vec<&str>> = Vec::new();
        let mut row_index = index + 2;
        let mut invalid_shape = false;
        while row_index < lines.len() {
            let line = &semantic_text[lines[row_index].0..lines[row_index].1];
            let Some(row) = markdown_pipe_row(line) else {
                break;
            };
            if row.len() != headers.len() || rows.len() >= 500 {
                invalid_shape = true;
                break;
            }
            rows.push(row);
            row_index += 1;
        }
        if !invalid_shape && !rows.is_empty() {
            let last = row_index - 1;
            tables.push(crate::SemanticMarkdownTable {
                start_utf8_byte: lines[index].0,
                end_utf8_byte_exclusive: lines[last].1,
                headers: headers.into_iter().map(str::to_owned).collect(),
                rows: rows
                    .into_iter()
                    .map(|row| row.into_iter().map(str::to_owned).collect())
                    .collect(),
            });
            index = row_index;
        } else {
            index += 1;
        }
    }
    tables
}

fn parse_exact_table(table: &str) -> Result<ParsedTable<'_>, ValidationError> {
    let mut lines = table.lines();
    let header_line = lines
        .next()
        .ok_or(ValidationError::Invalid("semantic table is empty"))?;
    let separator_line = lines.next().ok_or(ValidationError::Invalid(
        "semantic table separator is missing",
    ))?;
    let headers = markdown_pipe_row(header_line)
        .ok_or(ValidationError::Invalid("semantic table header is invalid"))?;
    let separator = markdown_pipe_row(separator_line).ok_or(ValidationError::Invalid(
        "semantic table separator is invalid",
    ))?;
    if headers.is_empty()
        || headers.len() > 20
        || separator.len() != headers.len()
        || !separator.iter().all(|cell| is_markdown_separator(cell))
    {
        return Err(ValidationError::Invalid(
            "semantic table header/separator mismatch",
        ));
    }
    let mut rows = Vec::new();
    for line in lines {
        let row = markdown_pipe_row(line)
            .ok_or(ValidationError::Invalid("semantic table row is invalid"))?;
        if row.len() != headers.len() || rows.len() >= 500 {
            return Err(ValidationError::Invalid(
                "semantic table row shape is invalid",
            ));
        }
        rows.push(row);
    }
    if rows.is_empty() {
        return Err(ValidationError::Invalid("semantic table has no data rows"));
    }
    Ok(ParsedTable { headers, rows })
}

fn markdown_pipe_row(line: &str) -> Option<Vec<&str>> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
        return None;
    }
    let inner = trimmed.strip_prefix('|')?.strip_suffix('|')?;
    let cells: Vec<&str> = inner.split('|').map(str::trim).collect();
    if cells.iter().any(|cell| cell.contains('\\')) {
        return None;
    }
    Some(cells)
}

fn is_markdown_separator(cell: &str) -> bool {
    let body = cell.strip_prefix(':').unwrap_or(cell);
    let body = body.strip_suffix(':').unwrap_or(body);
    body.len() >= 3 && body.bytes().all(|byte| byte == b'-')
}

pub(crate) fn table_matches_semantic_markdown(
    semantic_text: &str,
    headers: &[&str],
    rows: &[Vec<Value>],
) -> bool {
    let normalized = semantic_text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    for index in 0..lines.len().saturating_sub(1) {
        let Some(semantic_headers) = markdown_pipe_row(lines[index]) else {
            continue;
        };
        let Some(separator) = markdown_pipe_row(lines[index + 1]) else {
            continue;
        };
        if semantic_headers.len() != headers.len() || separator.len() != headers.len() {
            continue;
        }
        if semantic_headers
            .iter()
            .zip(headers)
            .any(|(left, right)| left != right)
            || !separator.iter().all(|cell| is_markdown_separator(cell))
        {
            continue;
        }
        let mut semantic_rows: Vec<Vec<&str>> = Vec::new();
        let mut row_index = index + 2;
        while row_index < lines.len() {
            let Some(row) = markdown_pipe_row(lines[row_index]) else {
                break;
            };
            if row.len() != headers.len() {
                semantic_rows.clear();
                break;
            }
            semantic_rows.push(row);
            row_index += 1;
        }
        if semantic_rows.len() == rows.len()
            && rows.iter().zip(&semantic_rows).all(|(row, expected)| {
                row.iter()
                    .zip(expected)
                    .all(|(cell, expected)| markdown_cell_text(cell) == *expected)
            })
        {
            return true;
        }
    }
    false
}

fn markdown_cell_text(cell: &Value) -> String {
    match cell {
        Value::Null => "—".to_owned(),
        Value::Bool(value) => value.to_string(),
        Value::Number(number) => number
            .as_f64()
            .and_then(es_number_string)
            .unwrap_or_else(|| number.to_string()),
        Value::String(value) => value.clone(),
        Value::Array(_) | Value::Object(_) => cell.to_string(),
    }
}

pub(crate) fn es_number_string(value: f64) -> Option<String> {
    if !value.is_finite() {
        return None;
    }
    if value == 0.0 {
        return Some("0".to_owned());
    }
    let formatted = format!("{:e}", value.abs());
    let (mantissa, exponent) = formatted.split_once('e')?;
    let exponent: i32 = exponent.parse().ok()?;
    let digits: String = mantissa
        .chars()
        .filter(|character| *character != '.')
        .collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let k = digits.len() as i32;
    let n = exponent + 1;
    let mut rendered = String::new();
    if k <= n && n <= 21 {
        rendered.push_str(digits);
        for _ in 0..(n - k) {
            rendered.push('0');
        }
    } else if 0 < n && n <= 21 {
        rendered.push_str(&digits[..n as usize]);
        rendered.push('.');
        rendered.push_str(&digits[n as usize..]);
    } else if -6 < n && n <= 0 {
        rendered.push_str("0.");
        for _ in 0..(-n) {
            rendered.push('0');
        }
        rendered.push_str(digits);
    } else {
        rendered.push_str(&digits[..1]);
        if k > 1 {
            rendered.push('.');
            rendered.push_str(&digits[1..]);
        }
        rendered.push('e');
        rendered.push(if exponent >= 0 { '+' } else { '-' });
        rendered.push_str(&exponent.abs().to_string());
    }
    Some(if value.is_sign_negative() {
        format!("-{rendered}")
    } else {
        rendered
    })
}
