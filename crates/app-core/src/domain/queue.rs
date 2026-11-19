//! Queue durability rules shared by the domain and persistence layers.
//!
//! The queue never advances on its own: ordering and removal are explicit
//! client actions. This module holds the database-free part of reorder
//! validation; the repository adds the existence and completeness checks that
//! need the store.

use super::{AppError, AppErrorCode};
use std::collections::HashSet;

/// Reject an order that names the same queue entry more than once.
///
/// A duplicated id would make dense renumbering ambiguous — one row would be
/// assigned two positions and an entry would be silently dropped.
pub fn validate_queue_order(ordered_ids: &[String]) -> Result<(), AppError> {
    let mut seen: HashSet<&str> = HashSet::with_capacity(ordered_ids.len());
    for id in ordered_ids {
        if !seen.insert(id.as_str()) {
            return Err(AppError::new(
                AppErrorCode::InvalidRequest,
                "duplicate queue id in reorder",
                false,
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_distinct_ids_and_empty_order() {
        assert!(validate_queue_order(&[]).is_ok());
        assert!(validate_queue_order(&["a".into(), "b".into(), "c".into()]).is_ok());
    }

    #[test]
    fn rejects_duplicated_ids() {
        let err = validate_queue_order(&["a".into(), "b".into(), "a".into()]).unwrap_err();
        assert_eq!(err.code, AppErrorCode::InvalidRequest);
        assert!(!err.retryable);
    }
}
