//! Dependency-free harness to compile and unit-test the REAL, UNMODIFIED
//! programs/solclash_events/src/math.rs from the SOLCLASH repo, in an
//! environment where crates.io (and therefore anchor-lang) is unreachable.
//!
//! What this verifies: the math logic and its inline #[cfg(test)] tests.
//! What this does NOT verify: anything involving anchor-lang — the real
//! errors.rs uses #[error_code]; here SolclashError is a plain enum stub
//! with only the variants math.rs uses, so the PartialEq-on-#[error_code]
//! assumption in the repo remains unverified.

pub mod constants {
    // Mirrors the four plain-u8 constants math.rs imports from the real
    // constants.rs (which itself can't compile here: it uses Pubkey).
    pub const CONDITION_GREATER_THAN: u8 = 0;
    pub const CONDITION_LESS_THAN: u8 = 1;
    pub const OUTCOME_NO: u8 = 0;
    pub const OUTCOME_YES: u8 = 1;
}

pub mod errors {
    // Stub of SolclashError with only the variants math.rs reaches.
    #[derive(Debug, PartialEq, Eq, Clone, Copy)]
    pub enum SolclashError {
        InvalidCondition,
        OraclePriceNonPositive,
        OracleExponentOutOfRange,
        MathOverflow,
        ZeroWinningStake,
        ZeroPot,
        ShareExceedsTotal,
    }
}

// The file under test, byte-for-byte the one committed on wip/offline-draft.
#[path = "../../../programs/solclash_events/src/math.rs"]
pub mod math;

/// Cross-language check: the committed tests/fixtures/payout.json and
/// refund.json were produced by an independent Python implementation
/// (generate_fixtures.py). Re-derive the same 50 deterministic stakes here
/// and assert the Rust compute_claim/compute_refund reach the exact totals
/// and remainders the Python run committed.
#[cfg(test)]
mod fixture_cross_check {
    use crate::math::{compute_claim, compute_refund};

    fn stakes() -> Vec<u64> {
        (0u64..50)
            .map(|i| 1_000_000 + (i * 37_919) % 4_500_000 + (i % 7) * 11_003)
            .collect()
    }

    #[test]
    fn payout_matches_python_fixture() {
        let stakes = stakes();
        let winning_stake: u64 = stakes.iter().sum();
        let payout_pool = 987_654_321_123u64;
        let total: u64 = stakes
            .iter()
            .map(|s| compute_claim(payout_pool, *s, winning_stake).unwrap())
            .sum();
        // Values committed in tests/fixtures/payout.json
        assert_eq!(total, 987_654_321_098);
        assert_eq!(payout_pool - total, 25);
    }

    #[test]
    fn refund_matches_python_fixture() {
        let stakes = stakes();
        let pot: u64 = stakes.iter().sum();
        let payout_pool = pot - 1_830_000; // RESOLVER_REWARD_DEV
        // Values committed in tests/fixtures/refund.json
        assert_eq!(payout_pool, 96_238_216);
        let total: u64 = stakes
            .iter()
            .map(|s| compute_refund(payout_pool, *s, pot).unwrap())
            .sum();
        assert_eq!(total, 96_238_190);
        assert_eq!(payout_pool - total, 26);
    }
}
