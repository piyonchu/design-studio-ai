//! Minimal pre-generation content gate: a keyword denylist that blocks clearly
//! disallowed prompts *before* spending on a model. Deliberately small,
//! deterministic, and dependency-free — not a substitute for a real moderation
//! model, but a cheap first line that stops obvious abuse pre-spend. Swap for a
//! moderation API behind this same `check_prompt` seam later.

use crate::error::AppError;

/// Illustrative denylist (case-insensitive substring match). The point is the
/// enforcement seam; a production deployment would expand this and/or call a
/// moderation model.
const DENY: &[&str] = &["csam", "child sexual", "bestiality", "how to make a bomb"];

/// Reject a prompt that hits the denylist. Called at enqueue and inside the
/// shared generation core, so both sync and async paths are covered.
pub fn check_prompt(prompt: &str) -> Result<(), AppError> {
    let p = prompt.to_lowercase();
    if DENY.iter().any(|term| p.contains(term)) {
        return Err(AppError::BadRequest(
            "prompt rejected by content policy".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::check_prompt;

    #[test]
    fn allows_normal_prompts() {
        assert!(check_prompt("a cozy mushroom house sprite").is_ok());
        assert!(check_prompt("hero knight, side view").is_ok());
    }

    #[test]
    fn blocks_denylisted_terms_case_insensitively() {
        assert!(check_prompt("CSAM something").is_err());
        assert!(check_prompt("How To Make A Bomb").is_err());
    }
}
