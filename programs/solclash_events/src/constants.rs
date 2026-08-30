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
//! `mainnet`-feature build. See the compile-time frozen-constants guard
//! (the `#[cfg(feature = "mainnet")] const _` block) below and
//! DEVIATIONS.md for the reasoning behind each dev value.

use anchor_lang::prelude::*;

/// Protocol fee, fixed by spec at 10% (1_000 bps). Not a TBD.
pub const PROTOCOL_FEE_BPS: u64 = 1_000;

/// Pyth Solana Receiver program (the receiver, NOT the Price Feed program).
/// Literal taken verbatim from the task spec.
pub const PYTH_RECEIVER_PROGRAM: Pubkey = pubkey!("rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ");

/// Minimum stake, must be `>> RESOLVER_REWARD` per spec. Dev value: exactly
/// 10x `RESOLVER_REWARD_DEV` (the frozen-constants guard asserts `>= 10x`,
/// the same number the doc states — a prior version's test checked `> 5x`,
/// inconsistent with this comment; aligned 2026-08-30).
///
/// NOTE for Fase 3: at two bettors the pot is `2 * MIN_STAKE = 36.6M` and
/// the reward `1.83M` — i.e. the reward is ~5% of the worst-case (2-bettor)
/// pot. If that overhead is judged too high, raise the multiple (50x would
/// put it under 1%); `RESOLVER_REWARD_DEV` itself is `_DEV` and will be
/// re-measured, so this is a Fase-3 decision, not changed now.
pub const MIN_STAKE_LAMPORTS_DEV: u64 = 18_300_000; // 0.0183 SOL = 10x RESOLVER_REWARD_DEV

/// Maximum single stake, a safety cap against a single wallet dominating a
/// pot enough to make rounding/overflow analysis meaningless. Dev value:
/// 1_000 SOL. TBD.
pub const MAX_STAKE_LAMPORTS_DEV: u64 = 1_000_000_000_000; // 1,000 SOL

/// Maximum pot size, a safety cap chosen so `u64` pot arithmetic and the
/// `u128` payout intermediate stay far from overflow even at
/// `MAX_STAKE_LAMPORTS_DEV` granularity. Dev value: 10,000,000 SOL
/// (10^16 lamports, well under `u64::MAX` ≈ 1.8 * 10^19). TBD.
pub const MAX_POT_LAMPORTS_DEV: u64 = 10_000_000_000_000_000; // 10,000,000 SOL

/// Destination of the protocol fee. Placeholder: a valid-but-obviously-fake
/// 32-byte address built from a readable ASCII marker
/// (`SOLCLASH-DEV-FEE-DO-NOT-SEND!!!!`, exactly 32 bytes — see
/// `FEE_WALLET_DEV_BYTES`). It is NOT a real fee wallet and must never
/// receive real funds; the byte string is human-legible in an account dump
/// precisely so no one mistakes it for one.
///
/// It is deliberately NOT the System Program: that address decodes to 32
/// bytes but is an EXECUTABLE account, and a lamport transfer to it does
/// not behave like a transfer to a normal wallet — the first definite
/// resolution would fail confusingly. (An earlier draft used a 41-character
/// all-ones string, which is 41 decoded bytes, not 32, so `pubkey!` — a
/// const macro — would have failed at compile time. Fixed 2026-08-30.)
/// TBD, covered by the frozen-constants compile-time guard below.
pub const FEE_WALLET_DEV_BYTES: [u8; 32] = *b"SOLCLASH-DEV-FEE-DO-NOT-SEND!!!!";
pub const FEE_WALLET_DEV: Pubkey = Pubkey::new_from_array(FEE_WALLET_DEV_BYTES);

/// Reward paid to the caller who successfully calls `resolve_event`.
/// Spec hint: "rent 0.00182 SOL + fee of 2+ tx". Dev value: 0.00182 SOL
/// (1_820_000 lamports) + 2 * 5_000 lamports (a commonly cited, NOT
/// verified-on-chain, per-signature base fee). TBD.
pub const RESOLVER_REWARD_DEV: u64 = 1_820_000 + 2 * 5_000; // 1,830,000 lamports

/// Minimum gap between `betting_close_time` and `resolution_time`. Dev
/// value: 300s (5 minutes). TBD.
pub const MIN_RESOLUTION_GAP_SECS_DEV: i64 = 300;

/// Maximum horizon of an event: `resolution_time` may be at most
/// `now + MAX_EVENT_HORIZON_SECS` at creation (a product policy, distinct
/// from the arithmetic overflow guard — see `create_event`). Dev value:
/// 30 days. Rationale: this program is a micro-event factory (horizons of
/// minutes to hours); a month covers any legitimate case ("BTC above X by
/// month end" is already the plausible ceiling) and keeps the worst-case
/// lockup — 30 days + RESOLUTION_TIMEOUT (7 days) = 37 days — within a
/// human timeframe. TBD: a positioning choice to confirm, not a
/// measurement.
pub const MAX_EVENT_HORIZON_SECS_DEV: i64 = 30 * 24 * 60 * 60;

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

/// Const byte-array equality, so the frozen-constants guard below can be
/// evaluated at COMPILE time (array `==` is not guaranteed usable in every
/// const position, and `Pubkey`'s `PartialEq` is not const — comparing the
/// raw `[u8; 32]` is).
const fn bytes32_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut i = 0;
    while i < 32 {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Frozen-constants guard — the mechanism the spec asked for, but as a
/// COMPILE-TIME check, not a `#[test]`.
///
/// A `#[test]` only runs under `cargo test`; `anchor build --features
/// mainnet` never runs the tests, so a test-based guard prevents nothing —
/// you can compile and deploy with every placeholder in place, and only
/// notice if someone remembers to run `cargo test --features mainnet`. A
/// `#[cfg(feature = "mainnet")] const _: () = { assert!(...) }` block is
/// evaluated during compilation, so a mainnet build with any placeholder
/// still in place FAILS TO COMPILE — the same enforcement the
/// `oracle-mock`/`mainnet` `compile_error!` guard uses. (A prior version
/// used a `#[test]`; fixed 2026-08-30.)
///
/// Each check is per-VALUE, and the whitelist check is per-ELEMENT on
/// purpose: an array-wide `!=` would pass if even one of the three feeds
/// were still a placeholder (say a real SOL/USD id with BTC and ETH left
/// as placeholders). A guard on a set must be written on the element, not
/// the set (fixed 2026-08-30).
#[cfg(feature = "mainnet")]
const _: () = {
    assert!(
        !bytes32_eq(&FEE_WALLET_DEV_BYTES, b"SOLCLASH-DEV-FEE-DO-NOT-SEND!!!!"),
        "FEE_WALLET_DEV placeholder still in place for a mainnet build"
    );
    assert!(
        !bytes32_eq(&FEED_WHITELIST_DEV[0], &[0u8; 32]),
        "FEED_WHITELIST_DEV[0] (SOL/USD) still a placeholder for a mainnet build"
    );
    assert!(
        !bytes32_eq(&FEED_WHITELIST_DEV[1], &[1u8; 32]),
        "FEED_WHITELIST_DEV[1] (BTC/USD) still a placeholder for a mainnet build"
    );
    assert!(
        !bytes32_eq(&FEED_WHITELIST_DEV[2], &[2u8; 32]),
        "FEED_WHITELIST_DEV[2] (ETH/USD) still a placeholder for a mainnet build"
    );
    assert!(
        MIN_STAKE_LAMPORTS_DEV != 18_300_000,
        "MIN_STAKE_LAMPORTS_DEV placeholder still in place for a mainnet build"
    );
    assert!(
        MAX_STAKE_LAMPORTS_DEV != 1_000_000_000_000,
        "MAX_STAKE_LAMPORTS_DEV placeholder still in place for a mainnet build"
    );
    assert!(
        MAX_POT_LAMPORTS_DEV != 10_000_000_000_000_000,
        "MAX_POT_LAMPORTS_DEV placeholder still in place for a mainnet build"
    );
    assert!(
        RESOLVER_REWARD_DEV != 1_830_000,
        "RESOLVER_REWARD_DEV placeholder still in place for a mainnet build"
    );
    assert!(
        MIN_RESOLUTION_GAP_SECS_DEV != 300,
        "MIN_RESOLUTION_GAP_SECS_DEV placeholder still in place for a mainnet build"
    );
    assert!(
        MAX_EVENT_HORIZON_SECS_DEV != 30 * 24 * 60 * 60,
        "MAX_EVENT_HORIZON_SECS_DEV placeholder still in place for a mainnet build"
    );
    assert!(
        MAX_RESOLUTION_STALENESS_SECS_DEV != 120,
        "MAX_RESOLUTION_STALENESS_SECS_DEV placeholder still in place for a mainnet build"
    );
    assert!(
        RESOLUTION_TIMEOUT_SECS_DEV != 7 * 24 * 60 * 60,
        "RESOLUTION_TIMEOUT_SECS_DEV placeholder still in place for a mainnet build"
    );
    assert!(
        CONF_MAX_RATIO_BPS_DEV != 500,
        "CONF_MAX_RATIO_BPS_DEV placeholder still in place for a mainnet build"
    );
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Sanity check on the dev seed data itself, runnable with plain
    /// `cargo test`. Aligned to the doc comment on `MIN_STAKE_LAMPORTS_DEV`
    /// (10x), where a prior version checked `> 5x` — same property, two
    /// numbers (fixed 2026-08-30).
    #[test]
    fn dev_min_stake_is_ten_times_resolver_reward() {
        assert!(MIN_STAKE_LAMPORTS_DEV >= RESOLVER_REWARD_DEV * 10);
    }

    /// The frozen-constants enforcement is now the compile-time `const _`
    /// block above, not a test — see its doc for why a `#[test]` guarded
    /// nothing during `anchor build --features mainnet`.
    #[test]
    fn frozen_guard_is_compile_time_not_a_test() {
        let source = include_str!("constants.rs");
        assert!(
            source.contains(r#"#[cfg(feature = "mainnet")]"#)
                && source.contains("const _: () = {")
                && source.contains("bytes32_eq(&FEED_WHITELIST_DEV[0]"),
            "the compile-time frozen-constants guard is missing from constants.rs"
        );
    }
}
