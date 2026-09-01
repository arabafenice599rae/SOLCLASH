//! Cross-checks `math.rs` against `tests/fixtures/*.json`.
//!
//! Those vectors were produced by `tests/fixtures/generate_fixtures.py`, an
//! independent re-implementation of the same formulas in plain Python. The
//! point of this file is that the two implementations are compared by a
//! machine rather than by eye: every case in every fixture is replayed
//! through the real Rust functions here, so a divergence between the spec's
//! arithmetic and this program's arithmetic fails the build.
//!
//! This is an integration test (`tests/`), so it sees only the crate's
//! public API — exactly the surface `oracle.rs` and the instruction
//! handlers consume.

use serde_json::Value;
use solclash_events::constants::{OUTCOME_NO, OUTCOME_YES};
use solclash_events::math::{
    compute_claim, compute_refund, normalize_to_e8, resolve_confidence_band,
};
use std::path::PathBuf;

/// `tests/fixtures/` lives at the workspace root, two levels above this
/// crate's manifest.
fn fixture(name: &str) -> Value {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path.push("tests/fixtures");
    path.push(name);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("fixture {} is not valid JSON: {e}", path.display()))
}

fn i128_at(case: &Value, key: &str) -> i128 {
    case[key]
        .as_i64()
        .unwrap_or_else(|| panic!("fixture field `{key}` is not an integer: {}", case[key]))
        as i128
}

fn u64_at(value: &Value, key: &str) -> u64 {
    value[key]
        .as_u64()
        .unwrap_or_else(|| panic!("fixture field `{key}` is not a u64: {}", value[key]))
}

fn u64_list(value: &Value, key: &str) -> Vec<u64> {
    value[key]
        .as_array()
        .unwrap_or_else(|| panic!("fixture field `{key}` is not an array"))
        .iter()
        .map(|v| {
            v.as_u64()
                .unwrap_or_else(|| panic!("fixture array `{key}` holds a non-u64: {v}"))
        })
        .collect()
}

#[test]
fn normalization_matches_fixture() {
    let doc = fixture("normalization.json");
    let cases = doc["cases"].as_array().expect("cases must be an array");
    assert!(!cases.is_empty(), "normalization.json has no cases");

    for case in cases {
        let name = case["name"].as_str().unwrap_or("<unnamed>");
        let value = i128_at(case, "value");
        let exponent = case["exponent"]
            .as_i64()
            .unwrap_or_else(|| panic!("{name}: exponent is not an integer"))
            as i32;
        let expected = i128_at(case, "expected_e8");

        // The fixture also carries the intermediate `shift`, so a
        // divergence points at *where* the two implementations parted.
        let expected_shift = case["shift"]
            .as_i64()
            .unwrap_or_else(|| panic!("{name}: shift is not an integer"));
        assert_eq!(
            expected_shift,
            exponent as i64 + 8,
            "{name}: fixture's own `shift` disagrees with exponent + 8"
        );

        let actual = normalize_to_e8(value, exponent)
            .unwrap_or_else(|e| panic!("{name}: normalize_to_e8 returned {e:?}"));
        assert_eq!(actual, expected, "{name}: normalize_to_e8 mismatch");
    }
}

#[test]
fn confidence_band_matches_fixture() {
    let doc = fixture("confidence_band.json");
    let cases = doc["cases"].as_array().expect("cases must be an array");
    assert!(!cases.is_empty(), "confidence_band.json has no cases");

    for case in cases {
        let name = case["name"].as_str().unwrap_or("<unnamed>");
        let condition = case["condition"]
            .as_u64()
            .unwrap_or_else(|| panic!("{name}: condition is not an integer"))
            as u8;
        let price_e8 = i128_at(case, "price_e8");
        let conf_e8 = i128_at(case, "conf_e8");
        let threshold_e8 = i128_at(case, "threshold_e8");

        assert_eq!(
            i128_at(case, "lower"),
            price_e8 - conf_e8,
            "{name}: fixture's own `lower` disagrees with price - conf"
        );
        assert_eq!(
            i128_at(case, "upper"),
            price_e8 + conf_e8,
            "{name}: fixture's own `upper` disagrees with price + conf"
        );

        // `null` is AMBIGUOUS, which is an outcome, not an error — see
        // the `candidate_outcome: None` doc on `state::Event`.
        let expected: Option<u8> = match &case["expected_outcome"] {
            Value::Null => None,
            v => {
                let n = v.as_u64().unwrap_or_else(|| {
                    panic!("{name}: expected_outcome is neither null nor an integer")
                }) as u8;
                assert!(
                    n == OUTCOME_YES || n == OUTCOME_NO,
                    "{name}: expected_outcome {n} is not a valid outcome"
                );
                Some(n)
            }
        };

        let actual = resolve_confidence_band(condition, price_e8, conf_e8, threshold_e8)
            .unwrap_or_else(|e| panic!("{name}: resolve_confidence_band returned {e:?}"));
        assert_eq!(actual, expected, "{name}: confidence band mismatch");
    }
}

#[test]
fn payout_matches_fixture() {
    let doc = fixture("payout.json");
    let payout_pool = u64_at(&doc, "payout_pool");
    let winning_stake = u64_at(&doc, "winning_stake");
    let stakes = u64_list(&doc, "stakes");
    let expected = u64_list(&doc, "expected_claims");

    assert_eq!(
        stakes.len(),
        expected.len(),
        "payout.json: stakes and expected_claims differ in length"
    );
    assert_eq!(
        stakes.iter().copied().sum::<u64>(),
        winning_stake,
        "payout.json: stakes do not sum to winning_stake"
    );

    let actual: Vec<u64> = stakes
        .iter()
        .map(|s| compute_claim(payout_pool, *s, winning_stake).expect("compute_claim failed"))
        .collect();
    assert_eq!(actual, expected, "compute_claim diverges from the fixture");

    // I11: the floor guarantees the pool is never over-drawn, and the
    // remainder is exactly what `close_event` sweeps.
    let total: u64 = actual.iter().copied().sum();
    assert_eq!(total, u64_at(&doc, "expected_total_paid"));
    assert!(
        total <= payout_pool,
        "I11 violated: payouts exceed the pool"
    );
    assert_eq!(
        payout_pool - total,
        u64_at(&doc, "expected_remainder_swept_by_close_event")
    );
}

#[test]
fn refund_matches_fixture() {
    let doc = fixture("refund.json");
    let pot = u64_at(&doc, "pot");
    let payout_pool = u64_at(&doc, "payout_pool");
    let stakes = u64_list(&doc, "stakes");
    let expected = u64_list(&doc, "expected_refunds");

    assert_eq!(
        stakes.len(),
        expected.len(),
        "refund.json: stakes and expected_refunds differ in length"
    );
    assert_eq!(
        stakes.iter().copied().sum::<u64>(),
        pot,
        "refund.json: stakes do not sum to pot"
    );
    assert_eq!(
        payout_pool + u64_at(&doc, "resolver_reward_dev"),
        pot,
        "refund.json: payout_pool + resolver reward should account for the whole pot"
    );

    let actual: Vec<u64> = stakes
        .iter()
        .map(|s| compute_refund(payout_pool, *s, pot).expect("compute_refund failed"))
        .collect();
    assert_eq!(actual, expected, "compute_refund diverges from the fixture");

    let total: u64 = actual.iter().copied().sum();
    assert_eq!(total, u64_at(&doc, "expected_total_paid"));
    assert!(
        total <= payout_pool,
        "I11 violated: refunds exceed the pool"
    );
    assert_eq!(
        payout_pool - total,
        u64_at(&doc, "expected_remainder_swept_by_close_event")
    );
}
