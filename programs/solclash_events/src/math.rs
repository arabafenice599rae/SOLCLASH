// STATUS: NEVER COMPILED — draft written without a
// toolchain. Fase 0 not performed. Nothing here is verified.

//! Pure functions only: e8 fixed-point normalization, confidence-band
//! outcome resolution, and pari-mutuel payout/refund arithmetic. Nothing
//! in this file touches an `Account`, a `Context`, or Pyth types — that is
//! deliberate, so it is the one file in this draft that is a plain Rust
//! library and can be unit-tested with `cargo test` alone, no Solana or
//! Anchor toolchain, no LiteSVM. Every reachable error path returns a
//! `SolclashError` — no `unwrap()`, no `expect()`, no un-checked arithmetic.
//!
//! `tests/fixtures/*.json` (Task B) hold independently-computed vectors
//! (via `tests/fixtures/generate_fixtures.py`, plain Python, no deps) for
//! comparison against `cargo test` output once a toolchain exists.

use crate::constants::{CONDITION_GREATER_THAN, CONDITION_LESS_THAN, OUTCOME_NO, OUTCOME_YES};
use crate::errors::SolclashError;

/// Normalizes a raw Pyth `(value, exponent)` pair to fixed-point 1e-8.
///
/// `shift = exponent + 8`. If `shift >= 0`, multiply by `10^shift`;
/// otherwise divide by `10^(-shift)`. The same function is used for both
/// `price` and `conf` — the spec is explicit that conf is normalized with
/// the *same* `exponent` as price, since Pyth reports one exponent per
/// update that applies to both fields.
///
/// Bounded and fully checked: `exponent` values that would make `10^n`
/// exceed what `i128` can hold return `OracleExponentOutOfRange` rather
/// than panicking, and the final scaling multiplication/division is
/// `checked_*` and returns `MathOverflow` on failure. `10^n` for `n <= 38`
/// always fits in `i128` (`i128::MAX` is ~1.7 * 10^38); realistic Pyth
/// exponents are single digits, so this bound is generous, not tight.
pub fn normalize_to_e8(value: i128, exponent: i32) -> Result<i128, SolclashError> {
    let shift = exponent
        .checked_add(8)
        .ok_or(SolclashError::OracleExponentOutOfRange)?;

    if shift >= 0 {
        let shift_u32 = shift as u32; // safe: shift >= 0 here
        let factor = 10i128
            .checked_pow(shift_u32)
            .ok_or(SolclashError::OracleExponentOutOfRange)?;
        value.checked_mul(factor).ok_or(SolclashError::MathOverflow)
    } else {
        let magnitude = shift
            .checked_neg()
            .ok_or(SolclashError::OracleExponentOutOfRange)?;
        let shift_u32 = magnitude as u32; // safe: magnitude > 0 here
        let factor = 10i128
            .checked_pow(shift_u32)
            .ok_or(SolclashError::OracleExponentOutOfRange)?;
        value.checked_div(factor).ok_or(SolclashError::MathOverflow)
    }
}

/// Resolves a candidate outcome from a normalized price, confidence, and
/// threshold, per the confidence-band rule (I12: never `Some` if
/// `[price - conf, price + conf]` straddles the threshold).
///
/// `GREATER_THAN` (spec-given, asymmetric on purpose):
/// - `price_e8 - conf_e8 > threshold_e8` -> `YES`
/// - `price_e8 + conf_e8 <= threshold_e8` -> `NO`
/// - otherwise -> `None` (AMBIGUOUS)
///
/// `LESS_THAN` is the mirror image: the strict/non-strict sides swap
/// because `LESS_THAN`'s YES condition is `price < threshold`, the
/// complement of `GREATER_THAN`'s:
/// - `price_e8 + conf_e8 < threshold_e8` -> `YES`
/// - `price_e8 - conf_e8 >= threshold_e8` -> `NO`
/// - otherwise -> `None` (AMBIGUOUS)
///
/// Returns `Ok(None)` for AMBIGUOUS — never an error. `condition` values
/// other than `CONDITION_GREATER_THAN`/`CONDITION_LESS_THAN` are rejected
/// by `create_event` (`InvalidCondition`) long before this is called, so
/// this function treats that case as a defensive `InvalidCondition` error
/// rather than a silent default.
pub fn resolve_confidence_band(
    condition: u8,
    price_e8: i128,
    conf_e8: i128,
    threshold_e8: i128,
) -> Result<Option<u8>, SolclashError> {
    let lower = price_e8
        .checked_sub(conf_e8)
        .ok_or(SolclashError::MathOverflow)?;
    let upper = price_e8
        .checked_add(conf_e8)
        .ok_or(SolclashError::MathOverflow)?;

    match condition {
        CONDITION_GREATER_THAN => {
            if lower > threshold_e8 {
                Ok(Some(OUTCOME_YES))
            } else if upper <= threshold_e8 {
                Ok(Some(OUTCOME_NO))
            } else {
                Ok(None)
            }
        }
        CONDITION_LESS_THAN => {
            if upper < threshold_e8 {
                Ok(Some(OUTCOME_YES))
            } else if lower >= threshold_e8 {
                Ok(Some(OUTCOME_NO))
            } else {
                Ok(None)
            }
        }
        _ => Err(SolclashError::InvalidCondition),
    }
}

/// `conf_e8 * 10_000 / price_e8 <= CONF_MAX_RATIO_BPS`, per spec step 8.
/// `price_e8` is assumed already checked `> 0` (spec step 6) by the caller;
/// this function still guards against a zero/negative `price_e8` defensively
/// rather than dividing by it blindly.
pub fn confidence_ratio_bps(price_e8: i128, conf_e8: i128) -> Result<u64, SolclashError> {
    if price_e8 <= 0 {
        return Err(SolclashError::OracleInvalidPrice);
    }
    let numerator = conf_e8
        .checked_mul(10_000)
        .ok_or(SolclashError::MathOverflow)?;
    let ratio = numerator
        .checked_div(price_e8)
        .ok_or(SolclashError::MathOverflow)?;
    u64::try_from(ratio).map_err(|_| SolclashError::MathOverflow)
}

/// Shared core for `compute_claim` and `compute_refund`: both are
/// `floor(payout_pool * share_stake / total_stake)` in a `u128`
/// intermediate. The floor is what makes `Σ payouts <= payout_pool`
/// (I11) true by construction — the last claimant can never fail for lack
/// of funds, and whatever remainder floor division leaves behind sits in
/// the PDA until `close_event` sweeps it.
///
/// `total_stake == 0` is rejected by the two callers (with distinct error
/// codes — `ZeroWinningStake` vs `ZeroPot` — since they mean different
/// things operationally) before this is ever reached, so this function
/// itself is `total_stake > 0` by contract, not by internal branching.
fn pro_rata_share(payout_pool: u64, share_stake: u64, total_stake: u64) -> Result<u64, SolclashError> {
    let numerator = (payout_pool as u128)
        .checked_mul(share_stake as u128)
        .ok_or(SolclashError::MathOverflow)?;
    let share = numerator
        .checked_div(total_stake as u128)
        .ok_or(SolclashError::MathOverflow)?;
    u64::try_from(share).map_err(|_| SolclashError::MathOverflow)
}

/// `claim_i = floor(payout_pool * stake_i / winning_stake)`.
pub fn compute_claim(payout_pool: u64, stake: u64, winning_stake: u64) -> Result<u64, SolclashError> {
    if winning_stake == 0 {
        return Err(SolclashError::ZeroWinningStake);
    }
    pro_rata_share(payout_pool, stake, winning_stake)
}

/// `refund_i = floor(payout_pool * stake_i / pot)`.
pub fn compute_refund(payout_pool: u64, stake: u64, pot: u64) -> Result<u64, SolclashError> {
    if pot == 0 {
        return Err(SolclashError::ZeroPot);
    }
    pro_rata_share(payout_pool, stake, pot)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- normalize_to_e8 ----

    #[test]
    fn normalize_exponent_minus_8_is_identity() {
        // shift = -8 + 8 = 0 -> price_e8 == price
        assert_eq!(normalize_to_e8(123_456_789, -8).unwrap(), 123_456_789);
    }

    #[test]
    fn normalize_exponent_minus_6_scales_up() {
        // shift = -6 + 8 = 2 -> multiply by 100
        assert_eq!(normalize_to_e8(15_000, -6).unwrap(), 1_500_000);
    }

    #[test]
    fn normalize_exponent_minus_9_scales_down() {
        // shift = -9 + 8 = -1 -> divide by 10
        assert_eq!(normalize_to_e8(1_234_567_890, -9).unwrap(), 123_456_789);
    }

    #[test]
    fn normalize_rejects_absurd_exponent() {
        assert_eq!(
            normalize_to_e8(1, 1_000),
            Err(SolclashError::OracleExponentOutOfRange)
        );
    }

    // ---- resolve_confidence_band ----

    #[test]
    fn greater_than_yes_strictly_above_band() {
        // price=110, conf=5, threshold=100 -> lower=105 > 100 -> YES
        assert_eq!(
            resolve_confidence_band(CONDITION_GREATER_THAN, 110, 5, 100).unwrap(),
            Some(OUTCOME_YES)
        );
    }

    #[test]
    fn greater_than_no_at_upper_boundary_is_inclusive() {
        // price=95, conf=5, threshold=100 -> upper=100 <= 100 -> NO
        assert_eq!(
            resolve_confidence_band(CONDITION_GREATER_THAN, 95, 5, 100).unwrap(),
            Some(OUTCOME_NO)
        );
    }

    #[test]
    fn greater_than_ambiguous_when_band_straddles() {
        // price=100, conf=5, threshold=100 -> lower=95 (not >100), upper=105 (not <=100)
        assert_eq!(
            resolve_confidence_band(CONDITION_GREATER_THAN, 100, 5, 100).unwrap(),
            None
        );
    }

    #[test]
    fn greater_than_lower_equal_threshold_is_ambiguous_not_yes() {
        // price=105, conf=5, threshold=100 -> lower=100, NOT > 100 -> falls through to ambiguous
        // (upper=110, not <= 100 either)
        assert_eq!(
            resolve_confidence_band(CONDITION_GREATER_THAN, 105, 5, 100).unwrap(),
            None
        );
    }

    #[test]
    fn less_than_is_mirror_of_greater_than() {
        // price=90, conf=5, threshold=100 -> upper=95 < 100 -> YES
        assert_eq!(
            resolve_confidence_band(CONDITION_LESS_THAN, 90, 5, 100).unwrap(),
            Some(OUTCOME_YES)
        );
        // price=105, conf=5, threshold=100 -> lower=100 >= 100 -> NO
        assert_eq!(
            resolve_confidence_band(CONDITION_LESS_THAN, 105, 5, 100).unwrap(),
            Some(OUTCOME_NO)
        );
        // price=100, conf=5, threshold=100 -> upper=105 (not <100), lower=95 (not >=100) -> AMBIGUOUS
        assert_eq!(
            resolve_confidence_band(CONDITION_LESS_THAN, 100, 5, 100).unwrap(),
            None
        );
    }

    #[test]
    fn invalid_condition_is_an_error_not_a_default() {
        assert_eq!(
            resolve_confidence_band(2, 100, 5, 100),
            Err(SolclashError::InvalidCondition)
        );
    }

    // ---- payout / refund ----

    #[test]
    fn claim_and_refund_share_the_same_floor_shape() {
        assert_eq!(compute_claim(1_000, 3, 7).unwrap(), 428); // floor(3000/7)
        assert_eq!(compute_refund(1_000, 3, 7).unwrap(), 428);
    }

    #[test]
    fn payout_sum_never_exceeds_pool_three_way_split() {
        // 1/2/3 stakes on the winning side, winning_stake = 6.
        let pool = 1_000u64;
        let winning_stake = 6u64;
        let stakes = [1u64, 2, 3];
        let claims: Vec<u64> = stakes
            .iter()
            .map(|s| compute_claim(pool, *s, winning_stake).unwrap())
            .collect();
        let total: u64 = claims.iter().sum();
        assert!(total <= pool);
        assert_eq!(claims, vec![166, 333, 500]);
        assert_eq!(total, 999); // 1 lamport left for close_event to sweep
    }

    #[test]
    fn zero_winning_stake_is_a_dedicated_error() {
        assert_eq!(
            compute_claim(1_000, 1, 0),
            Err(SolclashError::ZeroWinningStake)
        );
    }

    #[test]
    fn zero_pot_is_a_dedicated_error() {
        assert_eq!(compute_refund(1_000, 1, 0), Err(SolclashError::ZeroPot));
    }
}
