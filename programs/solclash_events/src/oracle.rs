// STATUS: NEVER COMPILED — draft written without a
// toolchain. Fase 0 not performed. Nothing here is verified.

//! The single Pyth boundary of the whole program. `resolve_event` is the
//! only caller of `verify_price_update`; the challenge mechanism was
//! removed once the update became provably unique on-chain (see below), so
//! there is no second copy of this verification to drift out of sync.
//!
//! # Canonicity replaces the publish-window + challenge design
//!
//! The old design accepted any update whose `publish_time` fell in a
//! `[resolution_time - 60, resolution_time]` window and relied on a
//! challenge round to converge on the newest one — a mitigation that
//! depended on someone posting a correction inside a short, unfunded
//! window (a security review finding, 2026-08-30). Pyth's own message
//! carries `prev_publish_time`, and for any instant `t` the unique update
//! is the one with `prev_publish_time < t <= publish_time` (see
//! docs/pyth-reference.md §2). Requiring exactly that here makes the
//! resolving update *provably* the canonical one for `resolution_time`,
//! with zero resolver discretion — so there is nothing to challenge.
//!
//! Fase 1 only wires the `oracle-mock` path: no `pyth-solana-receiver-sdk`
//! dependency exists in this draft at all. Fase 3 is expected to add a
//! second extraction function reading a real `PriceUpdateV2` (behind a
//! `mainnet`/non-mock feature) that produces the same `ExtractedPriceUpdate`
//! shape, so `verify_price_update` itself does not need to change.

use crate::constants::{CONF_MAX_RATIO_BPS_DEV, PYTH_RECEIVER_PROGRAM};
use crate::errors::SolclashError;
use crate::math::{confidence_ratio_bps, normalize_conf_to_e8, normalize_price_to_e8};
use anchor_lang::prelude::*;

/// Oracle data already reduced to the fields `verify_price_update` needs,
/// independent of whether it came from the mock or (in Fase 3) a real
/// `PriceUpdateV2`. This indirection is what lets both sources share one
/// verification function.
pub struct ExtractedPriceUpdate {
    /// Spec step 2: true iff the account's owner is `PYTH_RECEIVER_PROGRAM`.
    /// For a real `PriceUpdateV2`, this check is performed implicitly by
    /// `Account<'info, PriceUpdateV2>` at deserialization time (per spec:
    /// "lo fa Account<'info, PriceUpdateV2>"), so a real extraction path
    /// can set this unconditionally to `true` — reaching this struct at
    /// all already proves the owner matched. The mock has no such
    /// deserialization-time guarantee, so it computes this explicitly.
    pub owner_ok: bool,
    /// Spec step 3: true iff `verification_level == VerificationLevel::Full`.
    /// Asserted explicitly here rather than delegated to the SDK's
    /// `get_price_no_older_than`, which this program does not use.
    pub verification_full: bool,
    pub feed_id: [u8; 32],
    pub price: i64,
    pub conf: u64,
    pub exponent: i32,
    pub publish_time: i64,
    /// `publish_time` of the immediately preceding update for the same
    /// feed — the field that lets `verify_price_update` prove this update
    /// is the canonical one for `resolution_time`. From Pyth's
    /// `PriceFeedMessage.prev_publish_time` on the real path (Fase 3).
    pub prev_publish_time: i64,
}

/// Result of a successful `verify_price_update` call: a fully normalized,
/// confidence-checked price ready for `math::resolve_confidence_band`. The
/// staleness policy (how far after `resolution_time` the canonical update
/// may land before the outcome is treated as ambiguous) lives in
/// `resolve_event`, not here, because it changes the destination state
/// rather than rejecting the update — so `publish_time` is returned for
/// that caller to check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedPrice {
    pub price_e8: i128,
    pub conf_e8: i128,
    pub publish_time: i64,
}

/// Owner / verification / feed / canonicity / price / confidence checks,
/// in order. Status and timing preconditions, the confidence-band
/// outcome, the staleness policy, and the state writes are all
/// `resolve_event`'s job; this function is the shared oracle gate.
pub fn verify_price_update(
    update: &ExtractedPriceUpdate,
    event_feed_id: [u8; 32],
    resolution_time: i64,
) -> Result<VerifiedPrice> {
    require!(update.owner_ok, SolclashError::OracleOwnerMismatch);
    require!(
        update.verification_full,
        SolclashError::OracleVerificationNotFull
    );
    require!(
        update.feed_id == event_feed_id,
        SolclashError::OracleFeedMismatch
    );

    // Canonicity: prev_publish_time < resolution_time <= publish_time makes
    // this the UNIQUE Pyth update for the instant `resolution_time`. The
    // `<=` on the upper side means an update published exactly at
    // resolution_time resolves the event (boundary must succeed). No
    // window, no challenge — the resolver cannot pick among candidates.
    require!(
        update.publish_time >= resolution_time,
        SolclashError::OracleUpdateBeforeResolution
    );
    require!(
        update.prev_publish_time < resolution_time,
        SolclashError::OracleNotFirstAfterResolution
    );

    require!(update.price > 0, SolclashError::OraclePriceNonPositive);

    // step 7: same exponent for both fields, but different rounding on
    // the scale-down branch — price truncates (spec-literal formula),
    // conf rounds UP so rounding can only widen the band, never narrow
    // it toward a definite outcome (I12). See math.rs and DEVIATIONS.md.
    let price_e8 = normalize_price_to_e8(update.price as i128, update.exponent)?;
    let conf_e8 = normalize_conf_to_e8(update.conf as i128, update.exponent)?;

    // step 8: conf_e8 * 10_000 / price_e8 <= CONF_MAX_RATIO_BPS
    let ratio_bps = confidence_ratio_bps(price_e8, conf_e8)?;
    require!(
        ratio_bps <= CONF_MAX_RATIO_BPS_DEV,
        SolclashError::OracleConfidenceTooWide
    );

    Ok(VerifiedPrice {
        price_e8,
        conf_e8,
        publish_time: update.publish_time,
    })
}

/// `oracle-mock`: a fake account standing in for a real `PriceUpdateV2`,
/// used by every Fase 2 LiteSVM test. It intentionally carries an explicit
/// `owner` field, which a real `PriceUpdateV2` does not have — see the
/// `owner_ok` doc above for why that is necessary here and not in Fase 3.
#[cfg(feature = "oracle-mock")]
pub mod mock {
    use super::*;

    /// Mirrors the shape of `pyth_solana_receiver_sdk::price_update::VerificationLevel`
    /// (see docs/pyth-reference.md for the sourced definition this copies)
    /// closely enough to drive tests, without depending on the Pyth crate.
    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Debug)]
    pub enum MockVerificationLevel {
        Partial { num_signatures: u8 },
        Full,
    }

    impl MockVerificationLevel {
        pub fn is_full(&self) -> bool {
            matches!(self, MockVerificationLevel::Full)
        }
    }

    /// A `PriceUpdateV2`-shaped fake account. `owner` simulates the SPL
    /// account owner a real `PriceUpdateV2` would have (needed to express
    /// E9: "account owned by the Price Feed program instead of the
    /// receiver" as plain data, since the mock has no second on-chain
    /// program to actually own it).
    #[account]
    pub struct MockPriceUpdate {
        pub owner: Pubkey,
        pub verification_level: MockVerificationLevel,
        pub feed_id: [u8; 32],
        pub price: i64,
        pub conf: u64,
        pub exponent: i32,
        pub publish_time: i64,
        /// Mirrors Pyth's `PriceFeedMessage.prev_publish_time`; the mock
        /// carries it so canonicity tests can drive both the accepted case
        /// (`prev_publish_time < resolution_time <= publish_time`) and the
        /// two rejections.
        pub prev_publish_time: i64,
    }

    /// Reduces a `MockPriceUpdate` to the shape `verify_price_update`
    /// consumes, performing the mock-specific owner check that a real
    /// `Account<'info, PriceUpdateV2>` would perform implicitly.
    pub fn extract(update: &MockPriceUpdate) -> ExtractedPriceUpdate {
        ExtractedPriceUpdate {
            owner_ok: update.owner == PYTH_RECEIVER_PROGRAM,
            verification_full: update.verification_level.is_full(),
            feed_id: update.feed_id,
            price: update.price,
            conf: update.conf,
            exponent: update.exponent,
            publish_time: update.publish_time,
            prev_publish_time: update.prev_publish_time,
        }
    }
}
