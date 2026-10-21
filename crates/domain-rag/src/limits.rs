//! Bounded budgets for retrieval, attachment, and citation resolution.
//!
//! Every bound is a hard limit that fails closed. A request that exceeds a bound is
//! rejected or truncated with an explicit outcome; it is never silently widened.

use serde::{Deserialize, Serialize};

/// Maximum number of retrieved segments a single request may return.
pub const MAX_RESULTS: u16 = 20;
/// Maximum bytes of one excerpt copied out of a verified source revision.
pub const MAX_EXCERPT_BYTES: usize = 2_048;
/// Maximum aggregate excerpt bytes returned by one request.
pub const MAX_TOTAL_EXCERPT_BYTES: usize = 16_384;
/// Maximum query length accepted by a retrieval request.
pub const MAX_QUERY_BYTES: usize = 2_048;

/// Why a retrieval budget was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetrievalBudgetError {
    ResultCountOutOfRange,
    ExcerptTooLarge,
    TotalExcerptTooLarge,
    QueryTooLarge,
}

impl RetrievalBudgetError {
    pub fn reason_code(self) -> &'static str {
        match self {
            Self::ResultCountOutOfRange => "RETRIEVAL_RESULT_COUNT_OUT_OF_RANGE",
            Self::ExcerptTooLarge => "RETRIEVAL_EXCERPT_TOO_LARGE",
            Self::TotalExcerptTooLarge => "RETRIEVAL_TOTAL_EXCERPT_TOO_LARGE",
            Self::QueryTooLarge => "RETRIEVAL_QUERY_TOO_LARGE",
        }
    }

    pub fn reason(self) -> &'static str {
        match self {
            Self::ResultCountOutOfRange => "max_results must be between 1 and 20",
            Self::ExcerptTooLarge => "max_excerpt_bytes must not exceed 2048",
            Self::TotalExcerptTooLarge => "max_total_excerpt_bytes must not exceed 16384",
            Self::QueryTooLarge => "max_query_bytes must not exceed 2048",
        }
    }
}

/// Hard bounds applied to one retrieval request.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RetrievalBudget {
    pub max_results: u16,
    pub max_excerpt_bytes: usize,
    pub max_total_excerpt_bytes: usize,
    pub max_query_bytes: usize,
}

impl Default for RetrievalBudget {
    fn default() -> Self {
        Self {
            max_results: 10,
            max_excerpt_bytes: MAX_EXCERPT_BYTES,
            max_total_excerpt_bytes: MAX_TOTAL_EXCERPT_BYTES,
            max_query_bytes: MAX_QUERY_BYTES,
        }
    }
}

impl RetrievalBudget {
    /// The bounded budget used by the served local lexical provider.
    pub fn local_defaults() -> Self {
        Self::default()
    }

    pub fn validate(&self) -> Result<(), RetrievalBudgetError> {
        if self.max_results == 0 || self.max_results > MAX_RESULTS {
            return Err(RetrievalBudgetError::ResultCountOutOfRange);
        }
        if self.max_excerpt_bytes == 0 || self.max_excerpt_bytes > MAX_EXCERPT_BYTES {
            return Err(RetrievalBudgetError::ExcerptTooLarge);
        }
        if self.max_total_excerpt_bytes == 0
            || self.max_total_excerpt_bytes > MAX_TOTAL_EXCERPT_BYTES
            || self.max_total_excerpt_bytes < self.max_excerpt_bytes
        {
            return Err(RetrievalBudgetError::TotalExcerptTooLarge);
        }
        if self.max_query_bytes == 0 || self.max_query_bytes > MAX_QUERY_BYTES {
            return Err(RetrievalBudgetError::QueryTooLarge);
        }
        Ok(())
    }

    /// Fails closed when the query exceeds the request bound.
    pub fn validate_query(&self, query: &str) -> Result<(), RetrievalBudgetError> {
        if query.len() > self.max_query_bytes {
            return Err(RetrievalBudgetError::QueryTooLarge);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{RetrievalBudget, RetrievalBudgetError};

    #[test]
    fn default_budget_is_within_every_hard_limit() {
        let budget = RetrievalBudget::local_defaults();
        assert_eq!(budget.validate(), Ok(()));
    }

    #[test]
    fn zero_or_oversized_result_counts_are_rejected() {
        let mut budget = RetrievalBudget::local_defaults();
        budget.max_results = 0;
        assert_eq!(
            budget.validate(),
            Err(RetrievalBudgetError::ResultCountOutOfRange)
        );
        budget.max_results = 21;
        assert_eq!(
            budget.validate(),
            Err(RetrievalBudgetError::ResultCountOutOfRange)
        );
    }

    #[test]
    fn oversized_excerpt_and_total_bounds_are_rejected() {
        let mut budget = RetrievalBudget::local_defaults();
        budget.max_excerpt_bytes = 4_097;
        assert_eq!(
            budget.validate(),
            Err(RetrievalBudgetError::ExcerptTooLarge)
        );
        let mut budget = RetrievalBudget::local_defaults();
        budget.max_total_excerpt_bytes = 16_385;
        assert_eq!(
            budget.validate(),
            Err(RetrievalBudgetError::TotalExcerptTooLarge)
        );
    }

    #[test]
    fn a_total_budget_below_one_excerpt_is_rejected() {
        let mut budget = RetrievalBudget::local_defaults();
        budget.max_excerpt_bytes = 1_024;
        budget.max_total_excerpt_bytes = 512;
        assert_eq!(
            budget.validate(),
            Err(RetrievalBudgetError::TotalExcerptTooLarge)
        );
    }

    #[test]
    fn a_query_longer_than_the_bound_is_rejected() {
        let budget = RetrievalBudget::local_defaults();
        let query = "a".repeat(budget.max_query_bytes + 1);
        assert_eq!(
            budget.validate_query(&query),
            Err(RetrievalBudgetError::QueryTooLarge)
        );
        let query = "a".repeat(budget.max_query_bytes);
        assert_eq!(budget.validate_query(&query), Ok(()));
    }
}
