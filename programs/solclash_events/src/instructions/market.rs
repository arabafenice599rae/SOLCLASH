// STATUS: NEVER COMPILED — draft written without a
// toolchain. Fase 0 not performed. Nothing here is verified.

//! `create_event`, `place_bet`, `cancel_bet`, `lock_event` — everything
//! before an event enters resolution. All four are permissionless except
//! `cancel_bet`, which is restricted to the bet's own owner via the
//! `BetEntry` PDA's seeds (`["bet", event.key(), bettor.key()]") — no
//! manual owner check is written by hand here; see errors.rs for why.

use super::transfer_from_pda;
use crate::constants::*;
use crate::errors::SolclashError;
use crate::state::{BetEntry, Event, EventStatus};
use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

#[derive(Accounts)]
#[instruction(event_id: u64)]
pub struct CreateEvent<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,
    #[account(
        init,
        payer = creator,
        space = Event::SPACE,
        seeds = [b"event", creator.key().as_ref(), event_id.to_le_bytes().as_ref()],
        bump,
    )]
    pub event: Account<'info, Event>,
    pub system_program: Program<'info, System>,
}

pub fn create_event(
    ctx: Context<CreateEvent>,
    event_id: u64,
    feed_id: [u8; 32],
    condition: u8,
    threshold_e8: i128,
    betting_close_time: i64,
    resolution_time: i64,
) -> Result<()> {
    require!(
        FEED_WHITELIST_DEV.contains(&feed_id),
        SolclashError::FeedNotWhitelisted
    );
    require!(
        condition == CONDITION_GREATER_THAN || condition == CONDITION_LESS_THAN,
        SolclashError::InvalidCondition
    );

    let now = Clock::get()?.unix_timestamp;
    require!(
        betting_close_time > now,
        SolclashError::BettingCloseNotInFuture
    );
    let min_resolution_time = betting_close_time
        .checked_add(MIN_RESOLUTION_GAP_SECS_DEV)
        .ok_or(SolclashError::MathOverflow)?;
    require!(
        resolution_time >= min_resolution_time,
        SolclashError::ResolutionGapTooShort
    );

    let event = &mut ctx.accounts.event;
    event.creator = ctx.accounts.creator.key();
    event.event_id = event_id;
    event.feed_id = feed_id;
    event.condition = condition;
    event.threshold_e8 = threshold_e8;
    event.betting_close_time = betting_close_time;
    event.resolution_time = resolution_time;
    event.pot = 0;
    event.yes_stake = 0;
    event.no_stake = 0;
    event.bettor_count = 0;
    event.rent_exempt_minimum = Rent::get()?.minimum_balance(Event::SPACE);
    event.status = EventStatus::Open;
    event.candidate_outcome = None;
    event.candidate_price_e8 = 0;
    event.candidate_publish_time = 0;
    event.finalized_at = 0;
    event.payout_pool = 0;
    event.bets_closed = 0;
    // ASSUMPTION (unverified): `ctx.bumps.event` is Anchor 1.x's accessor
    // for the canonical bump of a PDA named `event` in this `Accounts`
    // struct. This is the modern (post-0.29) Anchor pattern; Fase 0 will
    // confirm or correct it immediately as a compile error if wrong.
    event.bump = ctx.bumps.event;

    Ok(())
}

#[derive(Accounts)]
pub struct PlaceBet<'info> {
    #[account(mut)]
    pub bettor: Signer<'info>,
    #[account(
        mut,
        seeds = [b"event", event.creator.as_ref(), event.event_id.to_le_bytes().as_ref()],
        bump = event.bump,
    )]
    pub event: Account<'info, Event>,
    #[account(
        init,
        payer = bettor,
        space = BetEntry::SPACE,
        seeds = [b"bet", event.key().as_ref(), bettor.key().as_ref()],
        bump,
    )]
    pub bet_entry: Account<'info, BetEntry>,
    pub system_program: Program<'info, System>,
}

pub fn place_bet(ctx: Context<PlaceBet>, outcome: u8, stake: u64) -> Result<()> {
    let event = &mut ctx.accounts.event;

    require!(event.status == EventStatus::Open, SolclashError::EventNotOpen);
    let now = Clock::get()?.unix_timestamp;
    require!(now < event.betting_close_time, SolclashError::BettingClosed);
    require!(
        outcome == OUTCOME_NO || outcome == OUTCOME_YES,
        SolclashError::InvalidOutcome
    );
    require!(stake >= MIN_STAKE_LAMPORTS_DEV, SolclashError::StakeTooLow);
    require!(stake <= MAX_STAKE_LAMPORTS_DEV, SolclashError::StakeTooHigh);

    let new_pot = event.pot.checked_add(stake).ok_or(SolclashError::MathOverflow)?;
    require!(
        new_pot <= MAX_POT_LAMPORTS_DEV,
        SolclashError::PotWouldExceedMax
    );

    transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.bettor.to_account_info(),
                to: event.to_account_info(),
            },
        ),
        stake,
    )?;

    event.pot = new_pot;
    if outcome == OUTCOME_YES {
        event.yes_stake = event
            .yes_stake
            .checked_add(stake)
            .ok_or(SolclashError::MathOverflow)?;
    } else {
        event.no_stake = event
            .no_stake
            .checked_add(stake)
            .ok_or(SolclashError::MathOverflow)?;
    }
    event.bettor_count = event
        .bettor_count
        .checked_add(1)
        .ok_or(SolclashError::MathOverflow)?;

    let bet_entry = &mut ctx.accounts.bet_entry;
    bet_entry.event = event.key();
    bet_entry.bettor = ctx.accounts.bettor.key();
    bet_entry.outcome = outcome;
    bet_entry.stake = stake;
    bet_entry.bump = ctx.bumps.bet_entry;

    let lamports = event.to_account_info().lamports();
    require!(
        lamports >= event.rent_exempt_minimum.checked_add(event.outstanding_liability()).ok_or(SolclashError::MathOverflow)?,
        SolclashError::EscrowMismatch
    );

    Ok(())
}

#[derive(Accounts)]
pub struct CancelBet<'info> {
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

pub fn cancel_bet(ctx: Context<CancelBet>) -> Result<()> {
    let event = &mut ctx.accounts.event;
    let bet_entry = &ctx.accounts.bet_entry;

    require!(event.status == EventStatus::Open, SolclashError::CancelNotOpen);

    let stake = bet_entry.stake;
    event.pot = event.pot.checked_sub(stake).ok_or(SolclashError::MathOverflow)?;
    if bet_entry.outcome == OUTCOME_YES {
        event.yes_stake = event
            .yes_stake
            .checked_sub(stake)
            .ok_or(SolclashError::MathOverflow)?;
    } else {
        event.no_stake = event
            .no_stake
            .checked_sub(stake)
            .ok_or(SolclashError::MathOverflow)?;
    }
    event.bettor_count = event
        .bettor_count
        .checked_sub(1)
        .ok_or(SolclashError::MathOverflow)?;

    transfer_from_pda(&event.to_account_info(), &ctx.accounts.bettor.to_account_info(), stake)?;

    let lamports = event.to_account_info().lamports();
    require!(
        lamports >= event.rent_exempt_minimum.checked_add(event.outstanding_liability()).ok_or(SolclashError::MathOverflow)?,
        SolclashError::EscrowMismatch
    );

    Ok(())
}

#[derive(Accounts)]
pub struct LockEvent<'info> {
    pub caller: Signer<'info>,
    #[account(
        mut,
        seeds = [b"event", event.creator.as_ref(), event.event_id.to_le_bytes().as_ref()],
        bump = event.bump,
    )]
    pub event: Account<'info, Event>,
}

pub fn lock_event(ctx: Context<LockEvent>) -> Result<()> {
    let event = &mut ctx.accounts.event;

    require!(event.status == EventStatus::Open, SolclashError::LockNotOpen);
    let now = Clock::get()?.unix_timestamp;
    require!(now >= event.betting_close_time, SolclashError::LockTooEarly);

    if event.yes_stake == 0 || event.no_stake == 0 {
        // One-sided book: refund everyone in full, no fee, no reward —
        // resolve_event is never entered on this path.
        event.status = EventStatus::Refundable;
        event.payout_pool = event.pot;
    } else {
        event.status = EventStatus::Locked;
    }

    let lamports = event.to_account_info().lamports();
    require!(
        lamports >= event.rent_exempt_minimum.checked_add(event.outstanding_liability()).ok_or(SolclashError::MathOverflow)?,
        SolclashError::EscrowMismatch
    );

    Ok(())
}
