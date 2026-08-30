// STATUS: NEVER COMPILED — draft written without a
// toolchain. Fase 0 not performed. Nothing here is verified.

//! The single Pyth boundary of the whole program. `resolve_event` and
//! `challenge_resolution` share ~90% of their price-update verification
//! (spec steps 2-8); duplicating that logic across two instruction
//! handlers is exactly the place where the two copies could drift apart
//! and turn into a vulnerability, so it lives here once, as
//! `verify_price_update`, and both instructions in `instructions/resolution.rs`
//! call it as a wrapper around their own instruction-specific pre/post
//! conditions (status checks, reward payment, candidate monotonicity).
//!
//! Fase 1 only wires the `oracle-mock` path: no `pyth-solana-receiver-sdk`
//! dependency exists in this draft at all. Fase 3 is expected to add a
//! second extraction function reading a real `PriceUpdateV2` (behind a
//! `mainnet`/non-mock feature) that produces the same `ExtractedPriceUpdate`
//! shape, so `verify_price_update` itself does not need to change.

use crate::constants::{CONF_MAX_RATIO_BPS_DEV, PUBLISH_WINDOW_SECS, PYTH_RECEIVER_PROGRAM};
use crate::errors::SolclashError;
use crate::math::{confidence_ratio_bps, normalize_to_e8};
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
}

/// Result of a successful `verify_price_update` call: a fully normalized,
/// confidence-checked price ready for `math::resolve_confidence_band`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedPrice {
    pub price_e8: i128,
    pub conf_e8: i128,
    pub publish_time: i64,
}

/// Spec steps 2-8, in order. Step 1 (status/timing preconditions) and step
/// 9-10 (confidence band, state writes, reward) are instruction-specific
/// and live in `instructions/resolution.rs`; this function is the shared
/// middle.
pub fn verify_price_update(
    update: &ExtractedPriceUpdate,
    event_feed_id: [u8; 32],
    resolution_time: i64,
) -> Result<VerifiedPrice> {
    require!(update.owner_ok, SolclashError::OracleOwnerMismatch); // step 2
    require!(
        update.verification_full,
        SolclashError::OracleVerificationNotFull
    ); // step 3
    require!(
        update.feed_id == event_feed_id,
        SolclashError::OracleFeedMismatch
    ); // step 4

    // step 5: publish_time <= resolution_time AND publish_time >= resolution_time - PUBLISH_WINDOW_SECS
    require!(
        update.publish_time <= resolution_time,
        SolclashError::OraclePublishTimeInFuture
    );
    let earliest_valid = resolution_time
        .checked_sub(PUBLISH_WINDOW_SECS)
        .ok_or(SolclashError::MathOverflow)?;
    require!(
        update.publish_time >= earliest_valid,
        SolclashError::OraclePublishTimeTooOld
    );

    require!(update.price > 0, SolclashError::OracleInvalidPrice); // step 6

    // step 7: normalize both price and conf with the same exponent
    let price_e8 = normalize_to_e8(update.price as i128, update.exponent)?;
    let conf_e8 = normalize_to_e8(update.conf as i128, update.exponent)?;

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
        }
    }
}
