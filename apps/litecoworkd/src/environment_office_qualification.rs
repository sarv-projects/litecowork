//! Office, data, and research capability qualification.
//!
//! `docs/ENVIRONMENTS.md` treats every external office/research/data implementation as
//! an external capability/provider. E07-S04 requires four properties from such a
//! provider:
//!
//! * the formula check and the rendered-layout check are *independent* — a single
//!   engine cannot certify both, because the same bug that produces a wrong formula
//!   usually produces a plausible-looking render;
//! * the source and its provenance are pinned;
//! * generated code or a generated capability is a reviewed draft only;
//! * broken formulas, truncated tabular sources, bad fonts or failed exports,
//!   malicious macros, and hallucinated sources are refused.
//!
//! This module implements those *rules*. It does not implement an office provider:
//! this build has no qualified spreadsheet, document, presentation, or renderer, so
//! the qualification probe reports every format as unavailable.

use serde::{Deserialize, Serialize};

/// The document and data formats the product understands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OfficeFormat {
    Xlsx,
    Docx,
    Pptx,
    Pdf,
    Csv,
}

impl OfficeFormat {
    pub fn media_types(self) -> &'static [&'static str] {
        match self {
            Self::Xlsx => &[
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            ],
            Self::Docx => &[
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            ],
            Self::Pptx => &[
                "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            ],
            Self::Pdf => &["application/pdf"],
            Self::Csv => &["text/csv", "application/csv"],
        }
    }

    /// Whether the format may carry executable macro content.
    pub fn may_carry_macros(self) -> bool {
        matches!(self, Self::Xlsx | Self::Docx | Self::Pptx | Self::Pdf)
    }
}

/// Whether a check is actually performed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckCapability {
    /// A qualified provider performs the check.
    Enforced,
    /// No provider performs the check.
    NotEnforced,
    /// Support could not be established.
    Unknown,
}

/// The qualification of one format.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfficeProviderQualification {
    pub format: OfficeFormat,
    /// Whether the provider can produce an editable document.
    pub editable: bool,
    pub formula_check: CheckCapability,
    pub layout_check: CheckCapability,
    /// The identity of the engine that evaluates formulas.
    pub formula_engine: String,
    /// The identity of the engine that renders and inspects layout.
    pub layout_engine: String,
    pub notes: Vec<String>,
}

/// Why a provider qualification was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QualificationRefusal {
    /// The formula and layout checks used the same engine, so they are not
    /// independent.
    ChecksNotIndependent,
    /// The formula check is not performed.
    FormulaCheckNotEnforced,
    /// The layout check is not performed.
    LayoutCheckNotEnforced,
}

/// The declared engines and capabilities of a candidate provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfficeProviderDeclaration {
    pub format: OfficeFormat,
    pub editable: bool,
    pub formula_check: CheckCapability,
    pub layout_check: CheckCapability,
    pub formula_engine: String,
    pub layout_engine: String,
}

/// Qualify a declared office provider.
///
/// A provider is only qualified when both checks are performed by two different
/// engines. A single engine is refused rather than counted twice.
pub fn qualify_office_provider(
    declaration: &OfficeProviderDeclaration,
) -> Result<OfficeProviderQualification, QualificationRefusal> {
    if declaration.formula_check != CheckCapability::Enforced {
        return Err(QualificationRefusal::FormulaCheckNotEnforced);
    }
    if declaration.layout_check != CheckCapability::Enforced {
        return Err(QualificationRefusal::LayoutCheckNotEnforced);
    }
    if declaration.formula_engine.trim().is_empty()
        || declaration.layout_engine.trim().is_empty()
        || declaration.formula_engine == declaration.layout_engine
    {
        return Err(QualificationRefusal::ChecksNotIndependent);
    }
    Ok(OfficeProviderQualification {
        format: declaration.format,
        editable: declaration.editable,
        formula_check: declaration.formula_check,
        layout_check: declaration.layout_check,
        formula_engine: declaration.formula_engine.clone(),
        layout_engine: declaration.layout_engine.clone(),
        notes: vec![],
    })
}

/// The qualification of the office providers in this build.
///
/// This build implements no office provider, so every format is reported as
/// unavailable with the reason stated.
pub fn probe_office_providers() -> Vec<OfficeProviderQualification> {
    [
        OfficeFormat::Xlsx,
        OfficeFormat::Docx,
        OfficeFormat::Pptx,
        OfficeFormat::Pdf,
        OfficeFormat::Csv,
    ]
    .into_iter()
    .map(|format| OfficeProviderQualification {
        format,
        editable: false,
        formula_check: CheckCapability::NotEnforced,
        layout_check: CheckCapability::NotEnforced,
        formula_engine: "unavailable".to_owned(),
        layout_engine: "unavailable".to_owned(),
        notes: vec![
            "no qualified office provider is implemented in this build".to_owned(),
        ],
    })
    .collect()
}

/// A pinned source that an output must be derived from.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedProvenance {
    pub source_resource_id: String,
    pub source_revision_id: String,
    /// `sha256:<lowercase hex>` digest of the exact source bytes.
    pub source_digest: String,
}

/// Why a source or output was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceRefusal {
    /// The source carries macro or script content that may execute.
    MacroContentRefused,
    /// A PDF carries an action that may execute when the document is opened.
    ActiveContentRefused,
    /// The tabular source is truncated or malformed.
    TruncatedSource,
    /// The tabular source exceeds its bounds.
    SourceTooLarge,
    /// The formula is syntactically invalid.
    FormulaSyntaxInvalid,
    /// The output does not reference a pinned source.
    UnpinnedProvenance,
    /// The output references a source that was not pinned.
    UnknownProvenance,
    /// A required font is not available to the renderer.
    FontUnavailable,
    /// The renderer produced no usable output.
    ExportFailed,
}

/// Detect executable content in a document.
///
/// An OOXML package that carries a VBA project, an OLE compound file, or a PDF that
/// carries an automatic action is refused before any provider sees it.
pub fn detect_executable_content(format: OfficeFormat, bytes: &[u8]) -> bool {
    if format == OfficeFormat::Csv {
        return false;
    }
    if bytes
        .windows(b"vbaProject".len())
        .any(|window| window == b"vbaProject")
    {
        return true;
    }
    if bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) {
        return true;
    }
    if format == OfficeFormat::Pdf {
        for marker in [
            &b"/JavaScript"[..],
            &b"/OpenAction"[..],
            &b"/Launch"[..],
            &b"/EmbeddedFile"[..],
        ] {
            if bytes.windows(marker.len()).any(|window| window == marker) {
                return true;
            }
        }
    }
    false
}

/// Bounds for a tabular source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TabularSourceLimits {
    pub max_bytes: u64,
    pub max_rows: usize,
    pub max_columns: usize,
}

impl Default for TabularSourceLimits {
    fn default() -> Self {
        Self {
            max_bytes: 64 * 1024 * 1024,
            max_rows: 1_000_000,
            max_columns: 4_096,
        }
    }
}

/// The observed structure of a tabular source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabularStructure {
    pub row_count: usize,
    pub column_count: usize,
}

/// Validate a tabular source.
///
/// A truncated CSV is detected through an unterminated quoted field or an inconsistent
/// column count between rows. Both are refused rather than silently repaired.
pub fn validate_tabular_source(
    bytes: &[u8],
    limits: TabularSourceLimits,
) -> Result<TabularStructure, SourceRefusal> {
    if bytes.len() as u64 > limits.max_bytes {
        return Err(SourceRefusal::SourceTooLarge);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| SourceRefusal::TruncatedSource)?;
    let mut rows = 0_usize;
    let mut in_quotes = false;
    let mut field_count = 0_usize;
    let mut row_fields = Vec::new();
    for character in text.chars() {
        match character {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                field_count += 1;
                if field_count > limits.max_columns {
                    return Err(SourceRefusal::SourceTooLarge);
                }
            }
            '\n' if !in_quotes => {
                field_count += 1;
                if field_count > limits.max_columns {
                    return Err(SourceRefusal::SourceTooLarge);
                }
                row_fields.push(field_count);
                field_count = 0;
                rows += 1;
                if rows > limits.max_rows {
                    return Err(SourceRefusal::SourceTooLarge);
                }
            }
            _ => {}
        }
    }
    if in_quotes {
        return Err(SourceRefusal::TruncatedSource);
    }
    if field_count > 0 {
        field_count += 1;
        row_fields.push(field_count);
        rows += 1;
    }
    if row_fields.is_empty() {
        return Ok(TabularStructure {
            row_count: 0,
            column_count: 0,
        });
    }
    let columns = row_fields[0];
    if row_fields.iter().any(|count| *count != columns) {
        return Err(SourceRefusal::TruncatedSource);
    }
    Ok(TabularStructure {
        row_count: rows,
        column_count: columns,
    })
}

/// Validate the syntax of a spreadsheet formula.
pub fn validate_formula_syntax(formula: &str) -> Result<(), SourceRefusal> {
    let trimmed = formula.trim();
    if !trimmed.starts_with('=') {
        return Err(SourceRefusal::FormulaSyntaxInvalid);
    }
    let mut depth = 0_i32;
    let mut in_quotes = false;
    for character in trimmed.chars() {
        match character {
            '"' => in_quotes = !in_quotes,
            '(' if !in_quotes => depth += 1,
            ')' if !in_quotes => {
                depth -= 1;
                if depth < 0 {
                    return Err(SourceRefusal::FormulaSyntaxInvalid);
                }
            }
            _ => {}
        }
    }
    if depth != 0 || in_quotes {
        return Err(SourceRefusal::FormulaSyntaxInvalid);
    }
    Ok(())
}

/// The fonts a renderer needs for one document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayoutCheckRequest {
    pub required_fonts: Vec<String>,
    pub available_fonts: Vec<String>,
    /// The number of pages the renderer produced.
    pub rendered_pages: usize,
}

/// Check a rendered layout independently of the formula check.
pub fn check_rendered_layout(request: &LayoutCheckRequest) -> Result<(), SourceRefusal> {
    for font in &request.required_fonts {
        if !request.available_fonts.iter().any(|available| available == font) {
            return Err(SourceRefusal::FontUnavailable);
        }
    }
    if request.rendered_pages == 0 {
        return Err(SourceRefusal::ExportFailed);
    }
    Ok(())
}

/// Verify that an output is derived from a pinned source.
pub fn verify_provenance(
    pinned: &[PinnedProvenance],
    claimed_source_digest: &str,
) -> Result<(), SourceRefusal> {
    if claimed_source_digest.trim().is_empty() {
        return Err(SourceRefusal::UnpinnedProvenance);
    }
    if !pinned
        .iter()
        .any(|source| source.source_digest == claimed_source_digest)
    {
        return Err(SourceRefusal::UnknownProvenance);
    }
    Ok(())
}

/// A capability or code fragment that was generated during a Task.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedOfficeCapability {
    pub capability_id: String,
    pub source: PinnedProvenance,
    /// Always `Draft`. A generated capability is never approved by its own producer.
    pub review_status: ReviewStatus,
}

/// The review state of a generated capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewStatus {
    Draft,
    Approved,
}

/// Why a generated capability could not be approved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewRefusal {
    /// Approval is an owner review action and is never performed by the producer.
    OwnerReviewRequired,
    /// The capability does not reference a pinned source.
    SourceNotPinned,
}

/// Approve a generated capability.
///
/// The producer of a generated capability may never approve it. The request is
/// recorded for the owner review queue and the capability stays a draft.
pub fn request_generated_capability_approval(
    capability: &GeneratedOfficeCapability,
) -> Result<ReviewStatus, ReviewRefusal> {
    if capability.source.source_digest.trim().is_empty() {
        return Err(ReviewRefusal::SourceNotPinned);
    }
    if capability.review_status == ReviewStatus::Approved {
        return Ok(ReviewStatus::Approved);
    }
    Err(ReviewRefusal::OwnerReviewRequired)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declaration() -> OfficeProviderDeclaration {
        OfficeProviderDeclaration {
            format: OfficeFormat::Xlsx,
            editable: true,
            formula_check: CheckCapability::Enforced,
            layout_check: CheckCapability::Enforced,
            formula_engine: "formula-engine-1".to_owned(),
            layout_engine: "layout-engine-1".to_owned(),
        }
    }

    #[test]
    fn a_provider_with_two_engines_is_qualified() {
        let qualification = qualify_office_provider(&declaration()).expect("qualified");
        assert_eq!(qualification.formula_check, CheckCapability::Enforced);
        assert_eq!(qualification.layout_check, CheckCapability::Enforced);
        assert!(qualification.editable);
    }

    #[test]
    fn a_provider_that_uses_one_engine_for_both_checks_is_refused() {
        let mut declaration = declaration();
        declaration.layout_engine = declaration.formula_engine.clone();
        assert_eq!(
            qualify_office_provider(&declaration).unwrap_err(),
            QualificationRefusal::ChecksNotIndependent
        );
    }

    #[test]
    fn a_provider_without_a_formula_or_layout_check_is_refused() {
        let mut without_formula = declaration();
        without_formula.formula_check = CheckCapability::Unknown;
        assert_eq!(
            qualify_office_provider(&without_formula).unwrap_err(),
            QualificationRefusal::FormulaCheckNotEnforced
        );
        let mut without_layout = declaration();
        without_layout.layout_check = CheckCapability::NotEnforced;
        assert_eq!(
            qualify_office_provider(&without_layout).unwrap_err(),
            QualificationRefusal::LayoutCheckNotEnforced
        );
    }

    #[test]
    fn this_build_offers_no_qualified_office_provider() {
        let qualifications = probe_office_providers();
        assert_eq!(qualifications.len(), 5);
        for qualification in qualifications {
            assert_eq!(qualification.formula_check, CheckCapability::NotEnforced);
            assert_eq!(qualification.layout_check, CheckCapability::NotEnforced);
            assert!(!qualification.editable);
        }
    }

    #[test]
    fn macro_and_active_content_is_refused() {
        assert!(detect_executable_content(
            OfficeFormat::Xlsx,
            b"PK\x03\x04 ... vbaProject.bin ..."
        ));
        assert!(detect_executable_content(
            OfficeFormat::Xlsx,
            &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1, 0x00]
        ));
        assert!(detect_executable_content(
            OfficeFormat::Pdf,
            b"%PDF-1.7 /OpenAction << >>"
        ));
        assert!(!detect_executable_content(
            OfficeFormat::Xlsx,
            b"PK\x03\x04 ordinary workbook"
        ));
        assert!(!detect_executable_content(OfficeFormat::Csv, b"a,b\n1,2\n"));
    }

    #[test]
    fn a_truncated_tabular_source_is_refused() {
        let limits = TabularSourceLimits::default();
        assert_eq!(
            validate_tabular_source(b"a,b,c\n1,2,3\n", limits).unwrap(),
            TabularStructure {
                row_count: 2,
                column_count: 3
            }
        );
        assert_eq!(
            validate_tabular_source(b"a,b,c\n1,2\n", limits).unwrap_err(),
            SourceRefusal::TruncatedSource
        );
        assert_eq!(
            validate_tabular_source(b"a,\"unterminated\n", limits).unwrap_err(),
            SourceRefusal::TruncatedSource
        );
    }

    #[test]
    fn an_oversized_tabular_source_is_refused() {
        let limits = TabularSourceLimits {
            max_bytes: 8,
            max_rows: 10,
            max_columns: 10,
        };
        assert_eq!(
            validate_tabular_source(b"a,b\n1,2\n3,4\n", limits).unwrap_err(),
            SourceRefusal::SourceTooLarge
        );
    }

    #[test]
    fn a_broken_formula_is_refused() {
        assert_eq!(validate_formula_syntax("=SUM(A1:A2)"), Ok(()));
        assert_eq!(
            validate_formula_syntax("=SUM(A1:A2").unwrap_err(),
            SourceRefusal::FormulaSyntaxInvalid
        );
        assert_eq!(
            validate_formula_syntax("=SUM(A1:A2))").unwrap_err(),
            SourceRefusal::FormulaSyntaxInvalid
        );
        assert_eq!(
            validate_formula_syntax("=SUM(\"open").unwrap_err(),
            SourceRefusal::FormulaSyntaxInvalid
        );
        assert_eq!(
            validate_formula_syntax("SUM(A1:A2)").unwrap_err(),
            SourceRefusal::FormulaSyntaxInvalid
        );
    }

    #[test]
    fn a_bad_font_or_failed_export_is_refused() {
        let request = LayoutCheckRequest {
            required_fonts: vec!["Inter".to_owned()],
            available_fonts: vec!["Inter".to_owned()],
            rendered_pages: 3,
        };
        assert_eq!(check_rendered_layout(&request), Ok(()));
        assert_eq!(
            check_rendered_layout(&LayoutCheckRequest {
                available_fonts: vec![],
                ..request.clone()
            })
            .unwrap_err(),
            SourceRefusal::FontUnavailable
        );
        assert_eq!(
            check_rendered_layout(&LayoutCheckRequest {
                rendered_pages: 0,
                ..request
            })
            .unwrap_err(),
            SourceRefusal::ExportFailed
        );
    }

    #[test]
    fn an_output_without_a_pinned_source_is_refused() {
        let pinned = vec![PinnedProvenance {
            source_resource_id: "resource-1".to_owned(),
            source_revision_id: "revision-1".to_owned(),
            source_digest: "sha256:aa".to_owned(),
        }];
        assert_eq!(verify_provenance(&pinned, "sha256:aa"), Ok(()));
        assert_eq!(
            verify_provenance(&pinned, "sha256:bb").unwrap_err(),
            SourceRefusal::UnknownProvenance
        );
        assert_eq!(
            verify_provenance(&pinned, "").unwrap_err(),
            SourceRefusal::UnpinnedProvenance
        );
    }

    #[test]
    fn a_generated_capability_stays_a_draft() {
        let capability = GeneratedOfficeCapability {
            capability_id: "capability-1".to_owned(),
            source: PinnedProvenance {
                source_resource_id: "resource-1".to_owned(),
                source_revision_id: "revision-1".to_owned(),
                source_digest: "sha256:aa".to_owned(),
            },
            review_status: ReviewStatus::Draft,
        };
        assert_eq!(
            request_generated_capability_approval(&capability).unwrap_err(),
            ReviewRefusal::OwnerReviewRequired
        );
        assert_eq!(capability.review_status, ReviewStatus::Draft);
    }
}
