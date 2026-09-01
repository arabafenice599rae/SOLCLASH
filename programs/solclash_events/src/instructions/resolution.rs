// STATUS: NEVER COMPILED — draft written without a
// toolchain. Fase 0 not performed. Nothing here is verified.

//! `resolve_event`, `challenge_resolution`, `finalize_resolution` — the
//! core of the program. `resolve_event` and `challenge_resolution` both
//! wrap `oracle::verify_price_update` (spec steps 2-8) and
//! `math::resolve_confidence_band` (step 9); only the instruction-specific
//! pre/post conditions (step 1, step 10) differ between the two, and are
//! written out separately below rather than sharing a further layer of
//! abstraction, since that is exactly the part the spec calls out as
//! legitimately different per instruction.
//!
//! This draft only implements the `oracle-mock` price source (Fase 1
//! scope). `price_update`'s account type is hard-wired to
//! `oracle::mock::MockPriceUpdate` rather than feature-gated, since no
//! alternative exists yet in this draft; Fase 3 will need to decide how
//! `resolve_event`/`challenge_resolution` pick between a mock and a real
//! `PriceUpdateV2` account type (two instruction variants, a generic
//! parameter, or a runtime-boxed abstraction) — see DEVIATIONS.md.

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
}

pub fn resolve_event(ctx: Context<ResolveEvent>) -> Result<()> {
    let event = &mut ctx.accounts.event;

    // step 1
    require!(
        event.status == EventStatus::Locked,
        SolclashError::EventNotLocked
    );
    let now = Clock::get()?.unix_timestamp;
    require!(now >= event.resolution_time, SolclashError::ResolveTooEarly);

    // steps 2-8
    let extracted = oracle::mock::extract(&ctx.accounts.price_update);
    let verified = oracle::verify_price_update(&extracted, event.feed_id, event.resolution_time)?;

    // step 9
    let outcome = resolve_confidence_band(
        event.condition,
        verified.price_e8,
        verified.conf_e8,
        event.threshold_e8,
    )?;

    // step 10
    event.candidate_outcome = outcome;
    event.candidate_price_e8 = verified.price_e8;
    event.candidate_publish_time = verified.publish_time;
    event.finalized_at = now
        .checked_add(RESOLUTION_CHALLENGE_SECS_DEV)
        .ok_or(SolclashError::MathOverflow)?;
    event.status = EventStatus::Resolving;

    // Escrow checkpoint at the exact instant of the Locked -> Resolving
    // transition, BEFORE paying RESOLVER_REWARD_DEV below. It must run
    // here, not after: once the reward leaves, the escrow permanently
    // holds `pot - RESOLVER_REWARD_DEV`, so the spec's literal
    // `Resolving => pot` formula (see `Event::outstanding_liability`)
    // could never be satisfied again for the rest of this event's
    // Resolving phase. See DEVIATIONS.md.
    let lamports = event.to_account_info().lamports();
    require!(
        lamports
            >= event
                .rent_exempt_minimum
                .checked_add(event.outstanding_liability())
                .ok_or(SolclashError::MathOverflow)?,
        SolclashError::EscrowMismatch
    );

    transfer_from_pda(
        &event.to_account_info(),
        &ctx.accounts.resolver.to_account_info(),
        RESOLVER_REWARD_DEV,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct ChallengeResolution<'info> {
    pub challenger: Signer<'info>,
    #[account(
        mut,
        seeds = [b"event", event.creator.as_ref(), event.event_id.to_le_bytes().as_ref()],
        bump = event.bump,
    )]
    pub event: Account<'info, Event>,
    pub price_update: Account<'info, MockPriceUpdate>,
}

pub fn challenge_resolution(ctx: Context<ChallengeResolution>) -> Result<()> {
    let event = &mut ctx.accounts.event;

    require!(
        event.status == EventStatus::Resolving,
        SolclashError::EventNotResolving
    );
    let now = Clock::get()?.unix_timestamp;
    require!(
        now < event.finalized_at,
        SolclashError::ChallengeWindowClosed
    );

    // steps 2-8, identical to resolve_event
    let extracted = oracle::mock::extract(&ctx.accounts.price_update);
    let verified = oracle::verify_price_update(&extracted, event.feed_id, event.resolution_time)?;

    // Challenge-specific: strictly newer publish_time than the current
    // candidate (I8 monotonicity). No reward.
    require!(
        verified.publish_time > event.candidate_publish_time,
        SolclashError::ChallengeNotNewer
    );

    // step 9
    let outcome = resolve_confidence_band(
        event.condition,
        verified.price_e8,
        verified.conf_e8,
        event.threshold_e8,
    )?;

    // Overwrite candidate. finalized_at is deliberately left untouched —
    // it never extends (I8), or an attacker could keep an event open
    // indefinitely by repeatedly challenging just before the deadline.
    event.candidate_outcome = outcome;
    event.candidate_price_e8 = verified.price_e8;
    event.candidate_publish_time = verified.publish_time;

    // No escrow checkpoint here: no lamports move in this instruction, and
    // by this point RESOLVER_REWARD_DEV has already permanently left the
    // escrow (in the resolve_event call that started this Resolving
    // phase), so the literal `Resolving => pot` formula cannot hold — see
    // the comment in resolve_event and DEVIATIONS.md.

    Ok(())
}

#[derive(Accounts)]
pub struct FinalizeResolution<'info> {
    pub caller: Signer<'info>,
    #[account(
        mut,
        seeds = [b"event", event.creator.as_ref(), event.event_id.to_le_bytes().as_ref()],
        bump = event.bump,
    )]
    pub event: Account<'info, Event>,
    /// I3: the protocol fee can only go to FEE_WALLET. Expressed as an
    /// Anchor `address` constraint rather than a manual `require!`, per
    /// the rule to prefer a constraint over a hand-written check when one
    /// exists. Unused (no lamports move here) on the AMBIGUOUS ->
    /// REFUNDABLE branch, but still required in every call for a single,
    /// uniform instruction shape.
    /// CHECK: no data is read from this account; only its address is
    /// checked, and it is only ever a lamport-transfer destination.
    #[account(mut, address = FEE_WALLET_DEV @ SolclashError::FeeWalletMismatch)]
    pub fee_wallet: UncheckedAccount<'info>,
}

pub fn finalize_resolution(ctx: Context<FinalizeResolution>) -> Result<()> {
    let event = &mut ctx.accounts.event;

    require!(
        event.status == EventStatus::Resolving,
        SolclashError::EventNotResolving
    );
    let now = Clock::get()?.unix_timestamp;
    require!(now >= event.finalized_at, SolclashError::FinalizeTooEarly);

    let pot_after_reward = event
        .pot
        .checked_sub(RESOLVER_REWARD_DEV)
        .ok_or(SolclashError::MathOverflow)?;

    match event.candidate_outcome {
        None => {
            // AMBIGUOUS candidate: full pro-rata refund, no protocol fee.
            event.status = EventStatus::Refundable;
            event.payout_pool = pot_after_reward;
        }
        Some(_) => {
            // protocol_fee = (pot - RESOLVER_REWARD) * PROTOCOL_FEE_BPS / 10_000,
            // floored — same u128 floor discipline as claim/refund, so
            // fee + payout_pool never exceeds pot_after_reward.
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

    // Escrow checkpoint at the terminal transition. Safe here regardless
    // of ordering relative to the fee payment above (unlike resolve_event):
    // `payout_pool` is exactly what's left after the fee, so
    // `lamports >= rent + payout_pool` holds both before and after paying
    // the fee (with slack for the fee amount before, none after). See
    // DEVIATIONS.md.
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
