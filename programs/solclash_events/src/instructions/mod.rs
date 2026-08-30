// STATUS: NEVER COMPILED — draft written without a
// toolchain. Fase 0 not performed. Nothing here is verified.

pub mod market;
pub mod resolution;
pub mod settlement;

pub use market::*;
pub use resolution::*;
pub use settlement::*;

use crate::errors::SolclashError;
use anchor_lang::prelude::*;

/// Moves `amount` lamports directly between two accounts owned by this
/// program (an `Event` PDA on one or both ends), without a System Program
/// CPI. This is safe and standard for a program moving lamports out of its
/// own PDA: only the owning program may mutate an account's lamport
/// balance directly, and a CPI is only required when the *source* is not
/// owned by the calling program (e.g. a wallet in `place_bet`, which uses
/// `system_program::transfer` instead — see `market.rs`).
///
/// Used for every Event-PDA-outbound movement: the resolver reward, the
/// protocol fee, individual claims/refunds, cancel_bet's stake return, and
/// close_event's final sweep. Fully checked, no silent wraparound.
pub(crate) fn transfer_from_pda<'info>(
    from: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    amount: u64,
) -> Result<()> {
    let mut from_lamports = from.try_borrow_mut_lamports()?;
    let mut to_lamports = to.try_borrow_mut_lamports()?;
    **from_lamports = from_lamports
        .checked_sub(amount)
        .ok_or(SolclashError::MathOverflow)?;
    **to_lamports = to_lamports
        .checked_add(amount)
        .ok_or(SolclashError::MathOverflow)?;
    Ok(())
}
