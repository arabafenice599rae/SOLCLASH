// STATUS: NEVER COMPILED — draft written without a
// toolchain. Fase 0 not performed. Nothing here is verified.

//! One dedicated variant per `require!` in the program. No shared/generic
//! error codes: every failure mode gets its own name so a transaction log
//! tells the caller exactly which invariant tripped.

use anchor_lang::prelude::*;

// ASSUMPTION (unverified — see DEVIATIONS.md): stacking `#[derive(PartialEq,
// Eq)]` above `#[error_code]` is assumed legal and sufficient to make
// `SolclashError` comparable with `assert_eq!`/`==` in math.rs's pure unit
// tests. anchor-lang's source was not available to confirm what
// `#[error_code]` derives on its own; if it already derives `PartialEq`,
// this is redundant and harmless, if it conflicts, Fase 0 will surface it
// immediately as a compile error and this attribute should be dropped.
#[derive(PartialEq, Eq)]
#[error_code]
pub enum SolclashError {
    // ---- create_event ----
    #[msg("feed_id is not in FEED_WHITELIST")]
    FeedNotWhitelisted,
    #[msg("condition must be 0 (GREATER_THAN) or 1 (LESS_THAN)")]
    InvalidCondition,
    #[msg("betting_close_time must be in the future")]
    BettingCloseNotInFuture,
    #[msg("resolution_time must be >= betting_close_time + MIN_RESOLUTION_GAP_SECS")]
    ResolutionGapTooShort,
    #[msg("resolution_time + RESOLUTION_TIMEOUT_SECS overflows i64: the timeout backstop would be uncomputable")]
    ResolutionTimeoutNotComputable,
    #[msg("resolution_time is beyond now + MAX_EVENT_HORIZON_SECS")]
    EventHorizonTooFar,

    // ---- place_bet ----
    #[msg("event is not OPEN")]
    EventNotOpen,
    #[msg("betting_close_time has passed")]
    BettingClosed,
    #[msg("outcome must be 0 (NO) or 1 (YES)")]
    InvalidOutcome,
    #[msg("stake is below MIN_STAKE_LAMPORTS")]
    StakeTooLow,
    #[msg("stake is above MAX_STAKE_LAMPORTS")]
    StakeTooHigh,
    #[msg("pot would exceed MAX_POT_LAMPORTS")]
    PotWouldExceedMax,

    // ---- cancel_bet ----
    #[msg("cancel_bet is only allowed while the event is OPEN")]
    CancelNotOpen,
    // No dedicated "wrong owner" / "wrong event" error: BetEntry's PDA
    // seeds are `["bet", event.key(), bettor.key()]`, so a mismatched
    // signer or a BetEntry from a different event fails Anchor's own
    // seeds constraint before the handler body ever runs. See
    // instructions/settlement.rs for where this is relied on instead of
    // a manual `require!`.

    // ---- lock_event ----
    #[msg("betting_close_time has not passed yet")]
    LockTooEarly,
    #[msg("event is not OPEN, cannot be locked")]
    LockNotOpen,

    // ---- resolve_event oracle checks ----
    #[msg("PriceUpdateV2 account is not owned by PYTH_RECEIVER_PROGRAM")]
    OracleOwnerMismatch,
    #[msg("PriceUpdateV2.verification_level is not Full")]
    OracleVerificationNotFull,
    #[msg("PriceUpdateV2.price_message.feed_id does not match event.feed_id")]
    OracleFeedMismatch,
    // Canonicity rule (replaces the old publish-window pair): the resolving
    // update is the UNIQUE Pyth update with prev_publish_time <
    // resolution_time <= publish_time — the first update at-or-after the
    // event's moment. The resolver has no discretion; either the submitted
    // update is that one, or it is rejected.
    #[msg("publish_time is before resolution_time (not the update at-or-after the event moment)")]
    OracleUpdateBeforeResolution,
    #[msg("prev_publish_time is at or after resolution_time (an earlier update is the canonical one)")]
    OracleNotFirstAfterResolution,
    #[msg("price is zero or negative")]
    OraclePriceNonPositive,
    #[msg("conf/price ratio exceeds CONF_MAX_RATIO_BPS")]
    OracleConfidenceTooWide,
    #[msg("exponent is out of the supported normalization range")]
    OracleExponentOutOfRange,

    // ---- resolve_event ----
    #[msg("event is not LOCKED")]
    EventNotLocked,
    #[msg("resolution_time has not passed yet")]
    ResolveTooEarly,
    #[msg("resolution window has closed (past resolution_time + RESOLUTION_TIMEOUT_SECS); only refund remains")]
    ResolutionWindowClosed,
    #[msg("fee_wallet does not match FEE_WALLET")]
    FeeWalletMismatch,
    // Retired with the challenge mechanism (2026-08-30): EventNotResolving,
    // ChallengeWindowClosed, ChallengeNotNewer, FinalizeTooEarly. The
    // canonicity check makes the resolving update unique, so there is no
    // Resolving phase, no challenge, and no finalize step to gate. See
    // DEVIATIONS.md and the error-code register in SECURITY.md.

    // ---- claim / claim_refund ----
    #[msg("event is not RESOLVED")]
    EventNotResolved,
    #[msg("event is not REFUNDABLE")]
    EventNotRefundable,
    // Retired with finalized_at (2026-08-30): ClaimBeforeFinalized. With
    // resolve_event going straight to a terminal state there is no
    // pre-finalize window, so I13 no longer exists and status == Resolved
    // / Refundable is the only gate claim/claim_refund need.
    // No NotWinningOutcome: on a Resolved event, claim is how EVERY
    // participant closes their position — a loser's share is simply zero
    // (rent back, no payout), so "loser tries to claim" is not an error
    // path. See instructions/settlement.rs and DEVIATIONS.md.

    // ---- mark_refundable ----
    #[msg("event is not eligible for the resolution timeout yet")]
    TimeoutNotReached,
    #[msg("event is not LOCKED, cannot be marked refundable by timeout")]
    MarkRefundableNotLocked,

    // ---- close_event ----
    #[msg("not every BetEntry has been claimed or refunded yet")]
    PayoutsNotComplete,
    #[msg("event is not in a terminal state (RESOLVED or REFUNDABLE)")]
    EventNotTerminal,

    // ---- shared invariants ----
    #[msg("escrow lamports are insufficient for rent-exempt minimum + outstanding liability")]
    EscrowMismatch,
    #[msg("arithmetic overflow or underflow")]
    MathOverflow,
    #[msg("winning_stake is zero, payout cannot be computed")]
    ZeroWinningStake,
    #[msg("pot is zero, refund cannot be computed")]
    ZeroPot,
    #[msg("share stake exceeds total stake, pro-rata payout refused")]
    ShareExceedsTotal,
}
