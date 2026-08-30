// STATUS: NEVER COMPILED — draft written without a
// toolchain. Fase 0 not performed. Nothing here is verified.

//! `claim`, `mark_refundable`, `claim_refund`, `close_event`.
//!
//! No escrow checkpoint (`Event::outstanding_liability` + `EscrowMismatch`)
//! is called inside `claim`/`claim_refund`: that helper's literal formula
//! (`Resolved | Refundable => event.payout_pool`) describes the total
//! liability at the moment of the terminal transition, not the remaining
//! balance after some claims have already been paid — it is checked once,
//! at that transition, in `finalize_resolution`/`lock_event`/
//! `mark_refundable` instead. Individual claims and refunds instead rely
//! on the floor-division proof in `math::pro_rata_share` (I11:
//! `Σ payouts <= payout_pool` by construction) for correctness. See
//! DEVIATIONS.md.

use super::transfer_from_pda;
use crate::constants::{OUTCOME_YES, RESOLUTION_TIMEOUT_SECS_DEV};
use crate::errors::SolclashError;
use crate::math::{compute_claim, compute_refund};
use crate::state::{BetEntry, Event, EventStatus};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Claim<'info> {
    #[account(mut)]
    pub bettor: Signer<'info>,
    #[account(
        mut,
        seeds = [b"event", event.creator.as_ref(), event.event_id.to_le_bytes().as_ref()],
        bump = event.bump,
    )]
    pub event: Account<'info, Event>,
    #[account(
        mut,
        close = bettor,
        seeds = [b"bet", event.key().as_ref(), bettor.key().as_ref()],
        bump = bet_entry.bump,
    )]
    pub bet_entry: Account<'info, BetEntry>,
}

pub fn claim(ctx: Context<Claim>) -> Result<()> {
    let event = &mut ctx.accounts.event;
    let bet_entry = &ctx.accounts.bet_entry;

    require!(event.status == EventStatus::Resolved, SolclashError::EventNotResolved);
    let now = Clock::get()?.unix_timestamp;
    // I13, defense-in-depth: status == Resolved already implies
    // now >= finalized_at transitively (finalize_resolution required it,
    // and finalized_at is immutable, and the on-chain clock never moves
    // backward), so this can never actually fire — kept anyway as a
    // direct, self-documenting assertion of the named invariant rather
    // than an indirect one.
    require!(now >= event.finalized_at, SolclashError::ClaimBeforeFinalized);

    // status == Resolved is only reached via finalize_resolution's `Some`
    // branch, which always leaves candidate_outcome as Some — this can
    // only be None here if that invariant were violated elsewhere.
    let winning_outcome = event
        .candidate_outcome
        .ok_or(SolclashError::EventNotResolved)?;
    require!(
        bet_entry.outcome == winning_outcome,
        SolclashError::NotWinningOutcome
    );

    let winning_stake = if winning_outcome == OUTCOME_YES {
        event.yes_stake
    } else {
        event.no_stake
    };
    let payout = compute_claim(event.payout_pool, bet_entry.stake, winning_stake)?;

    transfer_from_pda(&event.to_account_info(), &ctx.accounts.bettor.to_account_info(), payout)?;

    event.bets_closed = event
        .bets_closed
        .checked_add(1)
        .ok_or(SolclashError::MathOverflow)?;

    Ok(())
}

#[derive(Accounts)]
pub struct MarkRefundable<'info> {
    pub caller: Signer<'info>,
    #[account(
        mut,
        seeds = [b"event", event.creator.as_ref(), event.event_id.to_le_bytes().as_ref()],
        bump = event.bump,
    )]
    pub event: Account<'info, Event>,
}

pub fn mark_refundable(ctx: Context<MarkRefundable>) -> Result<()> {
    let event = &mut ctx.accounts.event;

    require!(
        event.status == EventStatus::Locked,
        SolclashError::MarkRefundableNotLocked
    );
    let now = Clock::get()?.unix_timestamp;
    let timeout_at = event
        .resolution_time
        .checked_add(RESOLUTION_TIMEOUT_SECS_DEV)
        .ok_or(SolclashError::MathOverflow)?;
    require!(now >= timeout_at, SolclashError::TimeoutNotReached);

    // resolve_event was never called on this path (status was still
    // Locked): no resolver reward was ever paid, so the full pot refunds.
    event.status = EventStatus::Refundable;
    event.payout_pool = event.pot;

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

#[derive(Accounts)]
pub struct ClaimRefund<'info> {
    #[account(mut)]
    pub bettor: Signer<'info>,
    #[account(
        mut,
        seeds = [b"event", event.creator.as_ref(), event.event_id.to_le_bytes().as_ref()],
        bump = event.bump,
    )]
    pub event: Account<'info, Event>,
    #[account(
        mut,
        close = bettor,
        seeds = [b"bet", event.key().as_ref(), bettor.key().as_ref()],
        bump = bet_entry.bump,
    )]
    pub bet_entry: Account<'info, BetEntry>,
}

pub fn claim_refund(ctx: Context<ClaimRefund>) -> Result<()> {
    let event = &mut ctx.accounts.event;
    let bet_entry = &ctx.accounts.bet_entry;

    require!(
        event.status == EventStatus::Refundable,
        SolclashError::EventNotRefundable
    );
    let now = Clock::get()?.unix_timestamp;
    // See the identical comment in `claim`. On the two paths that reach
    // Refundable without ever calling resolve_event (lock_event's
    // one-sided book, mark_refundable's timeout), finalized_at is still
    // its create_event default of 0, so this is trivially true there —
    // harmless, but noted in DEVIATIONS.md.
    require!(now >= event.finalized_at, SolclashError::ClaimBeforeFinalized);

    let refund = compute_refund(event.payout_pool, bet_entry.stake, event.pot)?;

    transfer_from_pda(&event.to_account_info(), &ctx.accounts.bettor.to_account_info(), refund)?;

    event.bets_closed = event
        .bets_closed
        .checked_add(1)
        .ok_or(SolclashError::MathOverflow)?;

    Ok(())
}

#[derive(Accounts)]
pub struct CloseEvent<'info> {
    pub caller: Signer<'info>,
    #[account(
        mut,
        close = creator,
        seeds = [b"event", event.creator.as_ref(), event.event_id.to_le_bytes().as_ref()],
        bump = event.bump,
    )]
    pub event: Account<'info, Event>,
    /// Recipient of the Event PDA's full remaining balance (rent-exempt
    /// minimum + whatever dust the floor-division remainders left behind)
    /// when the account closes. The spec names no recipient for this;
    /// `event.creator` was chosen because it is their market's leftover
    /// capital and no bettor has any further claim on it once every
    /// `BetEntry` has settled. See DEVIATIONS.md.
    /// CHECK: only a lamport-receiving destination; matched against
    /// event.creator via the `address` constraint.
    #[account(mut, address = event.creator)]
    pub creator: UncheckedAccount<'info>,
}

pub fn close_event(ctx: Context<CloseEvent>) -> Result<()> {
    let event = &ctx.accounts.event;

    require!(
        event.status == EventStatus::Resolved || event.status == EventStatus::Refundable,
        SolclashError::EventNotTerminal
    );
    require!(
        event.bets_closed == event.bettor_count,
        SolclashError::PayoutsNotComplete
    );

    // The actual lamport sweep is performed by Anchor's `close = creator`
    // constraint on `event` above during the account's exit routine — no
    // manual transfer needed here.
    Ok(())
}
