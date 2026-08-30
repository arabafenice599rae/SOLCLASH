// STATUS: NEVER COMPILED — draft written without a
// toolchain. Fase 0 not performed. Nothing here is verified.
// PARTIAL UPDATE 2026-08-30: this file's logic HAS now been compiled and
// its inline tests pass — including two pseudo-random property tests for
// I11/I12 and cross-checks against the Python-built fixtures — via the
// dependency-free harness in tools/math-harness, which stubs
// SolclashError. The anchor-lang integration (real errors.rs,
// #[error_code] + PartialEq) remains unverified; the header above still
// holds for the crate as a whole.

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

/// Computes the e8 scaling factor for a Pyth exponent: `shift = exponent
/// + 8`; returns `(scale_up, 10^|shift|)`. `exponent` values that would
/// make `10^n` exceed what `i128` can hold return
/// `OracleExponentOutOfRange` rather than panicking. `10^n` for `n <= 38`
/// always fits in `i128` (`i128::MAX` is ~1.7 * 10^38); realistic Pyth
/// exponents are single digits, so this bound is generous, not tight.
fn e8_shift_factor(exponent: i32) -> Result<(bool, i128), SolclashError> {
    let shift = exponent
        .checked_add(8)
        .ok_or(SolclashError::OracleExponentOutOfRange)?;

    if shift >= 0 {
        let factor = 10i128
            .checked_pow(shift as u32) // safe: shift >= 0 here
            .ok_or(SolclashError::OracleExponentOutOfRange)?;
        Ok((true, factor))
    } else {
        let magnitude = shift
            .checked_neg()
            .ok_or(SolclashError::OracleExponentOutOfRange)?;
        let factor = 10i128
            .checked_pow(magnitude as u32) // safe: magnitude > 0 here
            .ok_or(SolclashError::OracleExponentOutOfRange)?;
        Ok((false, factor))
    }
}

/// Normalizes a raw Pyth `price` to fixed-point 1e-8, truncating toward
/// zero on the scale-down branch — the literal formula the spec gives in
/// resolution step 7 (`checked_div`).
///
/// DO NOT use this for `conf` and DO NOT re-unify it with
/// `normalize_conf_to_e8` for tidiness: the two functions differ in
/// rounding direction ON PURPOSE. See `normalize_conf_to_e8` for why.
pub fn normalize_price_to_e8(value: i128, exponent: i32) -> Result<i128, SolclashError> {
    let (scale_up, factor) = e8_shift_factor(exponent)?;
    if scale_up {
        value.checked_mul(factor).ok_or(SolclashError::MathOverflow)
    } else {
        // factor is 10^n > 0, so checked_div here is structurally
        // unreachable as an error (i128 division only fails on division
        // by zero or i128::MIN / -1, neither possible with a positive
        // power of ten). Kept checked anyway for uniform style — there is
        // no hidden failure case a future reader needs to handle.
        value.checked_div(factor).ok_or(SolclashError::MathOverflow)
    }
}

/// Normalizes a raw Pyth `conf` to fixed-point 1e-8, rounding UP
/// (ceiling) on the scale-down branch.
///
/// `conf` must round up: truncating it toward zero NARROWS the confidence
/// band, and a narrower band makes the protocol MORE willing to declare a
/// definite YES/NO outcome near the threshold — the exact opposite of the
/// conservative direction I12 wants (never a `Some` outcome when the true
/// band straddles the threshold). Rounding conf up can only ever widen
/// the band, so rounding error can only push a borderline case toward
/// AMBIGUOUS, never toward a definite outcome. The magnitude is tiny (at
/// exponent -9, at most 0.9 e8-units), but the direction matters at the
/// boundary. This deliberately deviates from the spec's literal step 7
/// ("same normalization for conf") — see DEVIATIONS.md.
///
/// The scale-up branch is exact multiplication — no rounding happens, so
/// the two functions agree there.
pub fn normalize_conf_to_e8(value: i128, exponent: i32) -> Result<i128, SolclashError> {
    let (scale_up, factor) = e8_shift_factor(exponent)?;
    if scale_up {
        value.checked_mul(factor).ok_or(SolclashError::MathOverflow)
    } else {
        // Ceiling toward +infinity: quotient + 1 iff a positive value has
        // a nonzero remainder. For value <= 0 truncation already IS the
        // ceiling. (Pyth conf is a u64, so the live case is value >= 0.)
        let quotient = value.checked_div(factor).ok_or(SolclashError::MathOverflow)?;
        let remainder = value.checked_rem(factor).ok_or(SolclashError::MathOverflow)?;
        if remainder != 0 && value > 0 {
            quotient.checked_add(1).ok_or(SolclashError::MathOverflow)
        } else {
            Ok(quotient)
        }
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
        return Err(SolclashError::OraclePriceNonPositive);
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
///
/// `share_stake <= total_stake` is ALSO guaranteed by the callers today
/// (a BetEntry's stake is a summand of the total), but I11 is the
/// invariant guarding everyone's money, and this is the one place where
/// one line makes it true by construction instead of by contract: a
/// share exceeding the total would happily return more than
/// `payout_pool`, and `try_from` would not notice. Defense in depth, same
/// spirit as `overflow-checks`.
fn pro_rata_share(payout_pool: u64, share_stake: u64, total_stake: u64) -> Result<u64, SolclashError> {
    if share_stake > total_stake {
        return Err(SolclashError::ShareExceedsTotal);
    }
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

    // ---- normalize_price_to_e8 (truncating, spec step 7) ----

    #[test]
    fn price_exponent_minus_8_is_identity() {
        // shift = -8 + 8 = 0 -> price_e8 == price
        assert_eq!(normalize_price_to_e8(123_456_789, -8).unwrap(), 123_456_789);
    }

    #[test]
    fn price_exponent_minus_6_scales_up() {
        // shift = -6 + 8 = 2 -> multiply by 100.
        // 15_000 raw at exponent -6 is 0.015; at e8 scale that is 1_500_000.
        assert_eq!(normalize_price_to_e8(15_000, -6).unwrap(), 1_500_000);
    }

    #[test]
    fn price_exponent_minus_9_truncates_toward_zero() {
        // shift = -9 + 8 = -1 -> divide by 10, truncating
        assert_eq!(normalize_price_to_e8(1_234_567_890, -9).unwrap(), 123_456_789);
        assert_eq!(normalize_price_to_e8(1_234_567_895, -9).unwrap(), 123_456_789);
    }

    #[test]
    fn price_rejects_absurd_exponent() {
        assert_eq!(
            normalize_price_to_e8(1, 1_000),
            Err(SolclashError::OracleExponentOutOfRange)
        );
    }

    // ---- normalize_conf_to_e8 (ceiling on scale-down) ----

    #[test]
    fn conf_exact_division_matches_price_normalization() {
        // No remainder -> ceiling == truncation, the two functions agree.
        assert_eq!(normalize_conf_to_e8(50, -9).unwrap(), 5);
        assert_eq!(
            normalize_conf_to_e8(1_234_567_890, -9).unwrap(),
            normalize_price_to_e8(1_234_567_890, -9).unwrap()
        );
    }

    #[test]
    fn conf_inexact_division_rounds_up_not_down() {
        // 51 at exponent -9: truncation would give 5; ceiling gives 6.
        assert_eq!(normalize_conf_to_e8(51, -9).unwrap(), 6);
        // Even a 1-unit remainder rounds up: the band may only ever widen.
        assert_eq!(normalize_conf_to_e8(41, -9).unwrap(), 5);
        assert_eq!(normalize_conf_to_e8(9, -9).unwrap(), 1); // trunc would be 0
    }

    #[test]
    fn conf_scale_up_branch_is_exact_and_agrees_with_price() {
        assert_eq!(normalize_conf_to_e8(15_000, -6).unwrap(), 1_500_000);
        assert_eq!(normalize_conf_to_e8(0, -6).unwrap(), 0);
    }

    /// The boundary case the rounding direction exists for: with conf
    /// truncated, a triple that should be AMBIGUOUS becomes a definite
    /// YES. With conf ceiled, it stays AMBIGUOUS (I12).
    #[test]
    fn conf_ceiling_keeps_boundary_triple_ambiguous() {
        let threshold = 100i128;
        let price_e8 = 106i128;
        // raw conf 51 at exponent -9: truncation -> 5, ceiling -> 6
        let conf_truncated = normalize_price_to_e8(51, -9).unwrap();
        let conf_ceiled = normalize_conf_to_e8(51, -9).unwrap();
        assert_eq!(conf_truncated, 5);
        assert_eq!(conf_ceiled, 6);
        // Truncated conf: lower = 101 > 100 -> definite YES (the bias)
        assert_eq!(
            resolve_confidence_band(CONDITION_GREATER_THAN, price_e8, conf_truncated, threshold)
                .unwrap(),
            Some(OUTCOME_YES)
        );
        // Ceiled conf: lower = 100, not > 100 -> stays AMBIGUOUS
        assert_eq!(
            resolve_confidence_band(CONDITION_GREATER_THAN, price_e8, conf_ceiled, threshold)
                .unwrap(),
            None
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
        // price=105, conf=5, threshold=100 -> lower=100, NOT > 100 -> ambiguous
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
        // price=100, conf=5, threshold=100 -> straddles -> AMBIGUOUS
        assert_eq!(
            resolve_confidence_band(CONDITION_LESS_THAN, 100, 5, 100).unwrap(),
            None
        );
    }

    /// With conf = 0 the two conditions must be TOTAL: no uncertainty
    /// means AMBIGUOUS must be unreachable, for both conditions, at and
    /// around the threshold.
    #[test]
    fn zero_conf_never_yields_ambiguous() {
        let threshold = 100i128;
        for price in [98i128, 99, 100, 101, 102] {
            let gt = resolve_confidence_band(CONDITION_GREATER_THAN, price, 0, threshold).unwrap();
            let lt = resolve_confidence_band(CONDITION_LESS_THAN, price, 0, threshold).unwrap();
            assert!(gt.is_some(), "GT ambiguous at price {price} with conf 0");
            assert!(lt.is_some(), "LT ambiguous at price {price} with conf 0");
            // GREATER_THAN: YES iff price > threshold, NO otherwise
            assert_eq!(gt, Some(if price > threshold { OUTCOME_YES } else { OUTCOME_NO }));
            // LESS_THAN: YES iff price < threshold, NO otherwise
            assert_eq!(lt, Some(if price < threshold { OUTCOME_YES } else { OUTCOME_NO }));
        }
    }

    #[test]
    fn invalid_condition_is_an_error_not_a_default() {
        assert_eq!(
            resolve_confidence_band(2, 100, 5, 100),
            Err(SolclashError::InvalidCondition)
        );
    }

    // ---- confidence_ratio_bps ----

    #[test]
    fn ratio_below_at_and_above_a_threshold() {
        // conf/price = 5% exactly -> 500 bps
        assert_eq!(confidence_ratio_bps(10_000, 500).unwrap(), 500);
        // just below and just above
        assert_eq!(confidence_ratio_bps(10_000, 499).unwrap(), 499);
        assert_eq!(confidence_ratio_bps(10_000, 501).unwrap(), 501);
        // zero conf -> zero ratio
        assert_eq!(confidence_ratio_bps(10_000, 0).unwrap(), 0);
        // truncation: 1/3 of price -> 3333 bps, not 3334 (spec-literal
        // formula; direction flagged in DEVIATIONS.md)
        assert_eq!(confidence_ratio_bps(3, 1).unwrap(), 3333);
    }

    #[test]
    fn ratio_rejects_non_positive_price() {
        assert_eq!(
            confidence_ratio_bps(0, 1),
            Err(SolclashError::OraclePriceNonPositive)
        );
        assert_eq!(
            confidence_ratio_bps(-1, 1),
            Err(SolclashError::OraclePriceNonPositive)
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
    fn share_exceeding_total_is_rejected_not_overpaid() {
        assert_eq!(
            compute_claim(1_000, 8, 7),
            Err(SolclashError::ShareExceedsTotal)
        );
        assert_eq!(
            compute_refund(1_000, 8, 7),
            Err(SolclashError::ShareExceedsTotal)
        );
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

    // ---- pseudo-random properties (pure Rust, no external crates) ----

    /// Deterministic LCG so property tests are reproducible without any
    /// dependency. Knuth MMIX constants.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 11
        }
        fn in_range(&mut self, lo: u64, hi: u64) -> u64 {
            lo + self.next() % (hi - lo + 1)
        }
    }

    /// I12 as a property: for random (price, conf, threshold) triples and
    /// both conditions, a `Some` outcome implies the band does not
    /// straddle the threshold, `None` implies it does, and conf == 0
    /// implies the outcome is always `Some`.
    #[test]
    fn property_i12_band_never_straddles_on_some() {
        let mut rng = Lcg(0x50C1_C1A5_11E5_0001);
        for _ in 0..20_000 {
            let price = rng.in_range(1, 1_000_000_000_000_000) as i128;
            let conf = rng.in_range(0, 1_000_000_000_000) as i128;
            let threshold = rng.in_range(1, 1_000_000_000_000_000) as i128;
            let lower = price - conf;
            let upper = price + conf;
            for condition in [CONDITION_GREATER_THAN, CONDITION_LESS_THAN] {
                let outcome =
                    resolve_confidence_band(condition, price, conf, threshold).unwrap();
                match (condition, outcome) {
                    (CONDITION_GREATER_THAN, Some(o)) => {
                        if o == OUTCOME_YES {
                            assert!(lower > threshold);
                        } else {
                            assert!(upper <= threshold);
                        }
                    }
                    (CONDITION_LESS_THAN, Some(o)) => {
                        if o == OUTCOME_YES {
                            assert!(upper < threshold);
                        } else {
                            assert!(lower >= threshold);
                        }
                    }
                    (_, None) => {
                        // AMBIGUOUS must mean the band truly straddles:
                        // neither definite condition held.
                        assert!(conf > 0, "conf 0 must never be ambiguous");
                    }
                    _ => unreachable!(),
                }
                if conf == 0 {
                    assert!(outcome.is_some());
                }
            }
        }
    }

    /// I11 as a property over 50 heterogeneous pseudo-random stakes:
    /// the sum of floor-divided claims never exceeds the pool. Pure
    /// arithmetic counterpart of the LiteSVM E17/E18 plan entries.
    #[test]
    fn property_i11_fifty_random_stakes_never_overpay() {
        let mut rng = Lcg(0x50C1_C1A5_11E5_0002);
        for _round in 0..200 {
            let stakes: Vec<u64> =
                (0..50).map(|_| rng.in_range(1_000_000, 5_000_000_000)).collect();
            let winning_stake: u64 = stakes.iter().sum();
            let pool = rng.in_range(1, 10_000_000_000_000_000);
            let total: u128 = stakes
                .iter()
                .map(|s| compute_claim(pool, *s, winning_stake).unwrap() as u128)
                .sum();
            assert!(total <= pool as u128);
        }
    }
}
