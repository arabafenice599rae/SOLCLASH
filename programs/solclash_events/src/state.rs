// STATUS: NEVER COMPILED — draft written without a
// toolchain. Fase 0 not performed. Nothing here is verified.

use anchor_lang::prelude::*;

/// Market state machine. See the module doc on `Event::status` for the
/// full transition diagram.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum EventStatus {
    Open,
    Locked,
    Resolving,
    Resolved,
    Refundable,
}

/// One binary Yes/No prediction market.
///
/// Seeds: `["event", creator.key(), event_id.to_le_bytes()]`.
///
/// # `candidate_outcome: None` is not "unresolved"
///
/// While `status == Resolving`, `candidate_outcome == None` means the last
/// accepted price update landed inside the confidence band — an AMBIGUOUS
/// candidate, not a missing one. Whether resolution has even started is
/// carried entirely by `status`, never inferred from `candidate_outcome`.
/// An ambiguous candidate can still be overwritten by a later challenge
/// with a newer `publish_time`, exactly like a YES or NO candidate can.
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
    /// `None` while `status != Resolving`/`Resolved` has no meaning; while
    /// `Resolving`, `None` means "candidate is AMBIGUOUS" (see doc above).
    /// 0 = NO, 1 = YES. See `constants::OUTCOME_*`.
    pub candidate_outcome: Option<u8>,
    pub candidate_price_e8: i128,
    /// Monotonically non-decreasing while `Resolving`. Always
    /// `<= resolution_time` and `>= resolution_time - PUBLISH_WINDOW_SECS`.
    pub candidate_publish_time: i64,
    /// Set once, by `resolve_event`, to `now + RESOLUTION_CHALLENGE_SECS`.
    /// Immutable afterwards — a challenge overwrites the candidate but
    /// never extends this deadline.
    pub finalized_at: i64,
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
    /// ```text
    ///   8  discriminator          8  pot
    ///  32  creator                8  yes_stake
    ///   8  event_id               8  no_stake
    ///  32  feed_id                4  bettor_count
    ///   1  condition              8  rent_exempt_minimum
    ///  16  threshold_e8           1  status (5 unit variants -> 1-byte
    ///   8  betting_close_time        borsh discriminant)
    ///   8  resolution_time        2  candidate_outcome (Option<u8>:
    ///                                1-byte tag + <= 1 payload byte)
    ///  16  candidate_price_e8     8  finalized_at
    ///   8  candidate_publish_time 8  payout_pool
    ///   4  bets_closed            1  bump
    ///                           ---
    ///                           197
    /// ```
    pub const SPACE: usize = 8 + 32 + 8 + 32 + 1 + 16 + 8 + 8 + 8 + 8 + 8 + 4 + 8 + 1 + 2 + 16 + 8 + 8 + 8 + 4 + 1;

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
            EventStatus::Open | EventStatus::Locked | EventStatus::Resolving => self.pot,
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
    /// `8 (discriminator) + 32 (event) + 32 (bettor) + 1 (outcome)
    /// + 8 (stake) + 1 (bump)` = 82 bytes. Same hand-computed approach as
    /// `Event::SPACE` above, for the same reason.
    pub const SPACE: usize = 8 + 32 + 32 + 1 + 8 + 1;
}
