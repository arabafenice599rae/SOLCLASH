// STATUS: NEVER COMPILED — draft written without a
// toolchain. Fase 0 not performed. Nothing here is verified.

use anchor_lang::prelude::*;

/// Market state machine. See the module doc on `Event::status` for the
/// full transition diagram.
///
/// There is no `Resolving` state: `resolve_event` verifies the canonical
/// Pyth update (unique by `prev_publish_time < resolution_time <=
/// publish_time`) and moves straight to a terminal state — `Resolved` for
/// a definite YES/NO, `Refundable` for an ambiguous or stale outcome.
/// Because the update is unique there is nothing to challenge, so the old
/// resolve → challenge → finalize sequence collapses to one instruction.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum EventStatus {
    Open,
    Locked,
    Resolved,
    Refundable,
}

/// One binary Yes/No prediction market.
///
/// Seeds: `["event", creator.key(), event_id.to_le_bytes()]`.
///
/// # `resolved_outcome: None` on a terminal event
///
/// On a `Resolved` event `resolved_outcome` is always `Some(0|1)`. It is
/// `None` on a `Refundable` event (ambiguous/stale resolution, one-sided
/// book, or timeout) and on any pre-terminal event. Whether resolution
/// happened is carried entirely by `status`, never inferred from
/// `resolved_outcome`.
#[account]
pub struct Event {
    pub creator: Pubkey,
    pub event_id: u64,

    // ---- immutable market definition ----
    pub feed_id: [u8; 32],
    /// 0 = GREATER_THAN, 1 = LESS_THAN. See `constants::CONDITION_*`.
    pub condition: u8,
    pub threshold_e8: i128,
    pub betting_close_time: i64,
    pub resolution_time: i64,

    // ---- capital ----
    pub pot: u64,
    pub yes_stake: u64,
    pub no_stake: u64,
    pub bettor_count: u32,
    pub rent_exempt_minimum: u64,

    // ---- resolution ----
    pub status: EventStatus,
    /// The settled outcome, written once at the terminal transition.
    /// `Some(1)` = YES, `Some(0)` = NO on a `Resolved` event; `None` on a
    /// `Refundable` event (see the type-level doc above). 0 = NO, 1 = YES,
    /// per `constants::OUTCOME_*`.
    pub resolved_outcome: Option<u8>,
    /// The normalized price that settled the event, kept as an on-chain
    /// audit record of what `resolve_event` read (supports I10
    /// reconstructability). Zero on a `Refundable` event that never had a
    /// valid price (one-sided book, timeout).
    pub resolved_price_e8: i128,
    /// `publish_time` of the canonical Pyth update that settled the event.
    /// Zero on a `Refundable` event that never resolved.
    pub resolved_publish_time: i64,
    /// Written exactly once, at the OPEN/LOCKED -> terminal transition.
    pub payout_pool: u64,

    // ---- settlement bookkeeping ----
    /// Number of `BetEntry` accounts closed via `claim` or `claim_refund`.
    pub bets_closed: u32,

    pub bump: u8,
}

impl Event {
    /// Account space, computed by hand rather than via a possibly-absent
    /// `#[derive(InitSpace)]` (unverified whether that macro exists in
    /// this form in Anchor 1.x — anchor-lang's source was not available to
    /// this draft). One term per field, in declaration order, so this stays
    /// auditable against `Event`'s definition above:
    ///
    /// 8 (discriminator) + 32 (creator) + 8 (event_id) + 32 (feed_id)
    /// + 1 (condition) + 16 (threshold_e8) + 8 (betting_close_time)
    /// + 8 (resolution_time) + 8 (pot) + 8 (yes_stake) + 8 (no_stake)
    /// + 4 (bettor_count) + 8 (rent_exempt_minimum) + 1 (status: 4 unit
    /// variants, borsh-encodes as a 1-byte discriminant) + 2
    /// (resolved_outcome: Option<u8>, 1-byte tag + at most 1 payload byte)
    /// + 16 (resolved_price_e8) + 8 (resolved_publish_time)
    /// + 8 (payout_pool) + 4 (bets_closed) + 1 (bump)
    /// = 189 bytes (was 197; dropped finalized_at: 8 bytes).
    pub const SPACE: usize = 8 + 32 + 8 + 32 + 1 + 16 + 8 + 8 + 8 + 8 + 8 + 4 + 8 + 1 + 2 + 16 + 8 + 8 + 4 + 1;

    /// Lamports the PDA must hold on top of `rent_exempt_minimum`,
    /// depending on which phase of the state machine it is in. Before a
    /// terminal state, that liability is the whole pot (nothing has been
    /// paid out yet); after, it is whatever `payout_pool` still hasn't
    /// been claimed.
    ///
    /// I7 / invariant check: callers must assert
    /// `lamports >= rent_exempt_minimum + outstanding_liability(status)`,
    /// with `>=`, never `==` — see module doc in `instructions/settlement.rs`
    /// for why strict equality is unsafe (dust-lamport griefing).
    pub fn outstanding_liability(&self) -> u64 {
        match self.status {
            EventStatus::Open | EventStatus::Locked => self.pot,
            EventStatus::Resolved | EventStatus::Refundable => self.payout_pool,
        }
    }
}

/// One bettor's position in one event.
///
/// Seeds: `["bet", event.key(), bettor.key()]`. The PDA itself prevents a
/// second bet from the same wallet on the same event (the `init` constraint
/// fails on the second `place_bet`).
///
/// No `claimed` flag: `claim` and `claim_refund` both close this account
/// with `close = bettor`, which is simultaneously the anti-double-payout
/// guard (I6 — a closed account cannot be read or closed again) and the
/// rent refund to the bettor.
#[account]
pub struct BetEntry {
    pub event: Pubkey,
    pub bettor: Pubkey,
    /// 0 = NO, 1 = YES. See `constants::OUTCOME_*`.
    pub outcome: u8,
    pub stake: u64,
    pub bump: u8,
}

impl BetEntry {
    /// 8 (discriminator) + 32 (event) + 32 (bettor) + 1 (outcome)
    /// + 8 (stake) + 1 (bump) = 82 bytes. Same hand-computed approach as
    /// `Event::SPACE` above, for the same reason.
    pub const SPACE: usize = 8 + 32 + 32 + 1 + 8 + 1;
}
