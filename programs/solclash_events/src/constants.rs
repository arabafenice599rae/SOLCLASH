// STATUS: NEVER COMPILED — draft written without a
// toolchain. Fase 0 not performed. Nothing here is verified.

//! Protocol constants.
//!
//! Every `_DEV` value below is a development placeholder, not a researched
//! parameter. None of them were derived from a verifiable source (mainnet
//! fee-market data, a rent calculation performed against a running
//! validator, an actual Pyth Benchmarks latency measurement, etc.) because
//! this draft was written with no working toolchain and no network access
//! to Solana or Pyth infrastructure. Anything marked `TBD` in the spec is
//! carried here as `..._DEV` and MUST be re-derived for real before any
//! `mainnet`-feature build. See `mainnet_constants_are_frozen` below and
//! DEVIATIONS.md for the reasoning behind each dev value.

use anchor_lang::prelude::*;

/// Protocol fee, fixed by spec at 10% (1_000 bps). Not a TBD.
pub const PROTOCOL_FEE_BPS: u64 = 1_000;

/// Pyth Solana Receiver program (the receiver, NOT the Price Feed program).
/// Literal taken verbatim from the task spec.
pub const PYTH_RECEIVER_PROGRAM: Pubkey = pubkey!("rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ");

/// Minimum stake, must be `>> RESOLVER_REWARD` per spec. Dev value: 10x
/// `RESOLVER_REWARD_DEV`. TBD — not derived from real fee-market data.
pub const MIN_STAKE_LAMPORTS_DEV: u64 = 18_300_000; // 0.0183 SOL

/// Maximum single stake, a safety cap against a single wallet dominating a
/// pot enough to make rounding/overflow analysis meaningless. Dev value:
/// 1_000 SOL. TBD.
pub const MAX_STAKE_LAMPORTS_DEV: u64 = 1_000_000_000_000; // 1,000 SOL

/// Maximum pot size, a safety cap chosen so `u64` pot arithmetic and the
/// `u128` payout intermediate stay far from overflow even at
/// `MAX_STAKE_LAMPORTS_DEV` granularity. Dev value: 10,000,000 SOL
/// (10^16 lamports, well under `u64::MAX` ≈ 1.8 * 10^19). TBD.
pub const MAX_POT_LAMPORTS_DEV: u64 = 10_000_000_000_000_000; // 10,000,000 SOL

/// Destination of the protocol fee. Placeholder: the System Program's own
/// address (`11111111111111111111111111111111111111111`), chosen only
/// because it is a syntactically valid, publicly known, unowned-by-us
/// Pubkey — it is NOT a real fee wallet and must never receive real funds.
/// The spec says to ask for the real value "only when it's actually
/// needed"; per explicit instruction for this draft pass, it was not asked.
/// TBD, covered by `mainnet_constants_are_frozen`.
pub const FEE_WALLET_DEV: Pubkey = pubkey!("11111111111111111111111111111111111111111");

/// Reward paid to the caller who successfully calls `resolve_event`.
/// Spec hint: "rent 0.00182 SOL + fee of 2+ tx". Dev value: 0.00182 SOL
/// (1_820_000 lamports) + 2 * 5_000 lamports (a commonly cited, NOT
/// verified-on-chain, per-signature base fee). TBD.
pub const RESOLVER_REWARD_DEV: u64 = 1_820_000 + 2 * 5_000; // 1,830,000 lamports

/// Minimum gap between `betting_close_time` and `resolution_time`. Dev
/// value: 300s (5 minutes). TBD.
pub const MIN_RESOLUTION_GAP_SECS_DEV: i64 = 300;

/// Maximum staleness of the canonical resolution update: the gap between
/// `resolution_time` and the `publish_time` of the first Pyth update
/// at-or-after it. Under the canonicity rule the resolving update is the
/// unique one with `prev_publish_time < resolution_time <= publish_time`;
/// if the feed had an outage right at `resolution_time`, that unique
/// update can land far in the future, and settling on a price observed
/// long after the event's moment would be wrong. When
/// `publish_time - resolution_time` exceeds this cap the outcome is
/// treated as AMBIGUOUS and the event goes to REFUNDABLE — the same
/// conservative destination as a straddling confidence band. Dev value:
/// 120s (Pyth feeds normally update ~1/s, so a 2-minute gap means a real
/// outage, not jitter). TBD.
pub const MAX_RESOLUTION_STALENESS_SECS_DEV: i64 = 120;

/// If `LOCKED` sits past `resolution_time + RESOLUTION_TIMEOUT_SECS`
/// without ever being resolved, the event becomes `REFUNDABLE`. This is
/// also the UPPER bound of the resolution window: `resolve_event` is valid
/// only on `[resolution_time, resolution_time + RESOLUTION_TIMEOUT_SECS)`,
/// so resolve and the timeout-refund never overlap (they would otherwise
/// both be callable from `Locked`, and transaction ordering would decide
/// the payout regime — a security review finding, 2026-08-30). Dev value:
/// 7 days. TBD.
pub const RESOLUTION_TIMEOUT_SECS_DEV: i64 = 7 * 24 * 60 * 60;

/// Maximum allowed `conf_e8 * 10_000 / price_e8` ratio in bps before a
/// resolution is rejected with `OracleConfidenceTooWide`. Dev value: 500
/// bps (5%). TBD.
pub const CONF_MAX_RATIO_BPS_DEV: u64 = 500;

/// Whitelisted Pyth feed ids (SOL/USD, BTC/USD, ETH/USD). All-zero
/// placeholders: this draft has no verified access to Pyth's price feed id
/// registry (network to Pyth/pyth.network is blocked in this environment).
/// MUST be replaced with real feed ids in Fase 3, sourced from Pyth's
/// published feed list, not guessed. TBD.
pub const FEED_WHITELIST_DEV: [[u8; 32]; 3] = [
    [0u8; 32], // SOL/USD — placeholder, not a real feed id
    [1u8; 32], // BTC/USD — placeholder, not a real feed id
    [2u8; 32], // ETH/USD — placeholder, not a real feed id
];

/// GREATER_THAN / LESS_THAN encoding for `Event::condition`.
pub const CONDITION_GREATER_THAN: u8 = 0;
pub const CONDITION_LESS_THAN: u8 = 1;

/// YES / NO encoding for `BetEntry::outcome` and resolved outcomes.
pub const OUTCOME_NO: u8 = 0;
pub const OUTCOME_YES: u8 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    /// Sanity check on the dev seed data itself, runnable with plain
    /// `cargo test` and no Solana toolchain since it touches only `Pubkey`
    /// parsing and integer comparisons.
    #[test]
    fn dev_min_stake_dominates_resolver_reward() {
        assert!(MIN_STAKE_LAMPORTS_DEV > RESOLVER_REWARD_DEV * 5);
    }

    /// This is the test named in the spec. It must fail the build the
    /// moment the `mainnet` feature is turned on while any `_DEV` constant
    /// still holds its development value — i.e. nobody forgot to replace a
    /// placeholder before a mainnet build.
    ///
    /// NOTE: this can only run once the crate actually exists and Fase 0
    /// has produced a working `Cargo.toml` with a `mainnet` feature flag —
    /// it is written here as source, never executed.
    #[cfg(feature = "mainnet")]
    #[test]
    fn mainnet_constants_are_frozen() {
        assert_ne!(
            FEE_WALLET_DEV,
            pubkey!("11111111111111111111111111111111111111111"),
            "FEE_WALLET_DEV placeholder still in place for a mainnet build"
        );
        assert_ne!(
            FEED_WHITELIST_DEV,
            [[0u8; 32], [1u8; 32], [2u8; 32]],
            "FEED_WHITELIST_DEV placeholders still in place for a mainnet build"
        );
        assert_ne!(MIN_STAKE_LAMPORTS_DEV, 18_300_000);
        assert_ne!(MAX_STAKE_LAMPORTS_DEV, 1_000_000_000_000);
        assert_ne!(MAX_POT_LAMPORTS_DEV, 10_000_000_000_000_000);
        assert_ne!(RESOLVER_REWARD_DEV, 1_820_000 + 2 * 5_000);
        assert_ne!(MIN_RESOLUTION_GAP_SECS_DEV, 300);
        assert_ne!(MAX_RESOLUTION_STALENESS_SECS_DEV, 120);
        assert_ne!(RESOLUTION_TIMEOUT_SECS_DEV, 7 * 24 * 60 * 60);
        assert_ne!(CONF_MAX_RATIO_BPS_DEV, 500);
    }
}
