// STATUS: NEVER COMPILED — draft written without a
// toolchain. Fase 0 not performed. Nothing here is verified.

//! `resolve_event` — the whole of resolution, in one permissionless
//! instruction. It verifies the canonical Pyth update (unique by
//! `prev_publish_time < resolution_time <= publish_time`, see `oracle.rs`)
//! and moves straight to a terminal state:
//!
//! - definite YES/NO within the staleness cap -> `Resolved` (protocol fee
//!   charged, winner-take-all `payout_pool`);
//! - ambiguous confidence band, OR a canonical update staler than
//!   `MAX_RESOLUTION_STALENESS_SECS` (a feed outage right at
//!   `resolution_time`) -> `Refundable` (full pro-rata refund, no fee).
//!
//! There is no `Resolving` state, no `challenge_resolution`, and no
//! `finalize_resolution`. The old three-instruction sequence existed to
//! converge on the newest in-window update through a challenge round; once
//! the update is provably unique there is nothing to converge on, and the
//! challenge window was itself a griefing surface (security review,
//! 2026-08-30 — see DEVIATIONS.md). The resolver still earns
//! `RESOLVER_REWARD` for posting the update and paying the gas, on both
//! terminal branches, because the update is uniquely determined by
//! `resolution_time` and the feed — the resolver has no discretion to
//! grind, so paying on the ambiguous branch cannot be gamed.
//!
//! Fase 1 wires only the `oracle-mock` path (`price_update` is a
//! `MockPriceUpdate`); Fase 3 swaps in the real `PriceUpdateV2` behind the
//! same `ExtractedPriceUpdate` shape — see DEVIATIONS.md.

use super::transfer_from_pda;
use crate::constants::*;
use crate::errors::SolclashError;
use crate::math::resolve_confidence_band;
use crate::oracle::{self, mock::MockPriceUpdate};
use crate::state::{Event, EventStatus};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct ResolveEvent<'info> {
    #[account(mut)]
    pub resolver: Signer<'info>,
    #[account(
        mut,
        seeds = [b"event", event.creator.as_ref(), event.event_id.to_le_bytes().as_ref()],
        bump = event.bump,
    )]
    pub event: Account<'info, Event>,
    pub price_update: Account<'info, MockPriceUpdate>,
    /// I3: the protocol fee can only go to FEE_WALLET. Anchor `address`
    /// constraint rather than a manual `require!`. Only receives lamports
    /// on the `Resolved` branch; still required on every call for a single
    /// uniform instruction shape.
    /// CHECK: no data is read from this account; only its address is
    /// checked, and it is only ever a lamport-transfer destination.
    #[account(mut, address = FEE_WALLET_DEV @ SolclashError::FeeWalletMismatch)]
    pub fee_wallet: UncheckedAccount<'info>,
}

pub fn resolve_event(ctx: Context<ResolveEvent>) -> Result<()> {
    let event = &mut ctx.accounts.event;

    // Preconditions. The upper bound makes the resolution window
    // `[resolution_time, resolution_time + RESOLUTION_TIMEOUT_SECS)`
    // disjoint from `mark_refundable`'s `[.. + timeout, inf)`, so the two
    // are never simultaneously callable from `Locked` and transaction
    // ordering can't pick the payout regime (security review, 2026-08-30).
    require!(event.status == EventStatus::Locked, SolclashError::EventNotLocked);
    let now = Clock::get()?.unix_timestamp;
    require!(now >= event.resolution_time, SolclashError::ResolveTooEarly);
    let window_end = event
        .resolution_time
        .checked_add(RESOLUTION_TIMEOUT_SECS_DEV)
        .ok_or(SolclashError::MathOverflow)?;
    require!(now < window_end, SolclashError::ResolutionWindowClosed);

    // Oracle gate: owner / verification / feed / canonicity / price / conf.
    let extracted = oracle::mock::extract(&ctx.accounts.price_update);
    let verified = oracle::verify_price_update(&extracted, event.feed_id, event.resolution_time)?;

    // Staleness: canonicity guarantees publish_time >= resolution_time, so
    // this gap is >= 0. If the canonical update lands more than
    // MAX_RESOLUTION_STALENESS_SECS after the event's moment, the feed had
    // an outage at resolution_time and no honest price exists — treat as
    // ambiguous (-> Refundable), the same conservative destination as a
    // straddling band.
    let staleness = verified
        .publish_time
        .checked_sub(event.resolution_time)
        .ok_or(SolclashError::MathOverflow)?;
    let outcome = if staleness > MAX_RESOLUTION_STALENESS_SECS_DEV {
        None
    } else {
        resolve_confidence_band(
            event.condition,
            verified.price_e8,
            verified.conf_e8,
            event.threshold_e8,
        )?
    };

    // On-chain audit record of what settled the event (I10).
    event.resolved_price_e8 = verified.price_e8;
    event.resolved_publish_time = verified.publish_time;
    event.resolved_outcome = outcome;

    let pot_after_reward = event
        .pot
        .checked_sub(RESOLVER_REWARD_DEV)
        .ok_or(SolclashError::MathOverflow)?;

    match outcome {
        None => {
            // Ambiguous or stale: full pro-rata refund, no protocol fee.
            event.status = EventStatus::Refundable;
            event.payout_pool = pot_after_reward;
        }
        Some(_) => {
            // protocol_fee = (pot - RESOLVER_REWARD) * PROTOCOL_FEE_BPS /
            // 10_000, floored — same u128 floor discipline as claim/refund,
            // so fee + payout_pool never exceeds pot_after_reward.
            let fee_u128 = (pot_after_reward as u128)
                .checked_mul(PROTOCOL_FEE_BPS as u128)
                .ok_or(SolclashError::MathOverflow)?
                .checked_div(10_000)
                .ok_or(SolclashError::MathOverflow)?;
            let protocol_fee = u64::try_from(fee_u128).map_err(|_| SolclashError::MathOverflow)?;
            let payout_pool = pot_after_reward
                .checked_sub(protocol_fee)
                .ok_or(SolclashError::MathOverflow)?;

            event.status = EventStatus::Resolved;
            event.payout_pool = payout_pool;

            transfer_from_pda(
                &event.to_account_info(),
                &ctx.accounts.fee_wallet.to_account_info(),
                protocol_fee,
            )?;
        }
    }

    // Reward the resolver on both branches (see module doc: no discretion,
    // so no grinding to game).
    transfer_from_pda(
        &event.to_account_info(),
        &ctx.accounts.resolver.to_account_info(),
        RESOLVER_REWARD_DEV,
    )?;

    // Escrow checkpoint on the final resting state: after the fee and
    // reward have left, the PDA holds exactly `rent + payout_pool`, so the
    // `>=` (never `==`, I7) invariant holds with any pre-funded dust as
    // slack.
    let lamports = event.to_account_info().lamports();
    require!(
        lamports
            >= event
                .rent_exempt_minimum
                .checked_add(event.outstanding_liability())
                .ok_or(SolclashError::MathOverflow)?,
        SolclashError::EscrowMismatch
    );

    Ok(())
}
