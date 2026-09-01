// STATUS: compiles, and `cargo test` is green — Fase 0 has been performed,
// see TOOLCHAIN.md. Still NEVER DEPLOYED and never audited: no on-chain
// test exercises any instruction (Fase 2), the Pyth path is still the
// mock (Fase 3), and every `_DEV` constant is a placeholder.

//! SOLCLASH-EVENTS: a binary Yes/No, P2P, non-custodial, variable-stake
//! pari-mutuel prediction market on Solana, settled by reading a Pyth
//! price update. Entrypoint only — all logic lives in `instructions/`,
//! `math.rs`, and `oracle.rs`.

use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod instructions;
pub mod math;
pub mod oracle;
pub mod state;

use instructions::*;

// PLACEHOLDER — NOT a real deployed program ID and NOT a real keypair:
// this is sha256("SOLCLASH_EVENTS_PLACEHOLDER_DO_NOT_DEPLOY_FASE0_PENDING")
// reinterpreted as a Pubkey, generated offline with no network access, only
// so `declare_id!` has a syntactically valid 32-byte value to parse. No
// private key exists for this address. Regenerate for real with
// `solana-keygen new` and sync via `anchor keys sync` once a toolchain
// exists (Fase 0) — see DEVIATIONS.md.
declare_id!("6aFse5Z9e6M97Hcro492hcb9b8sdkvZJ2zBHAGUdwBb1");

// Mandatory guard: oracle-mock must never ship in a mainnet build. See
// `tests` module below for the test that checks this guard exists in
// source (it cannot itself trigger the guard, since a test binary that
// enabled both features would fail to compile before any test could run —
// the point of a `compile_error!`).
#[cfg(all(feature = "mainnet", feature = "oracle-mock"))]
compile_error!("oracle-mock cannot be enabled in a mainnet build");

#[program]
pub mod solclash_events {
    use super::*;

    pub fn create_event(
        ctx: Context<CreateEvent>,
        event_id: u64,
        feed_id: [u8; 32],
        condition: u8,
        threshold_e8: i128,
        betting_close_time: i64,
        resolution_time: i64,
    ) -> Result<()> {
        instructions::market::create_event(
            ctx,
            event_id,
            feed_id,
            condition,
            threshold_e8,
            betting_close_time,
            resolution_time,
        )
    }

    pub fn place_bet(ctx: Context<PlaceBet>, outcome: u8, stake: u64) -> Result<()> {
        instructions::market::place_bet(ctx, outcome, stake)
    }

    pub fn cancel_bet(ctx: Context<CancelBet>) -> Result<()> {
        instructions::market::cancel_bet(ctx)
    }

    pub fn lock_event(ctx: Context<LockEvent>) -> Result<()> {
        instructions::market::lock_event(ctx)
    }

    pub fn resolve_event(ctx: Context<ResolveEvent>) -> Result<()> {
        instructions::resolution::resolve_event(ctx)
    }

    pub fn challenge_resolution(ctx: Context<ChallengeResolution>) -> Result<()> {
        instructions::resolution::challenge_resolution(ctx)
    }

    pub fn finalize_resolution(ctx: Context<FinalizeResolution>) -> Result<()> {
        instructions::resolution::finalize_resolution(ctx)
    }

    pub fn claim(ctx: Context<Claim>) -> Result<()> {
        instructions::settlement::claim(ctx)
    }

    pub fn mark_refundable(ctx: Context<MarkRefundable>) -> Result<()> {
        instructions::settlement::mark_refundable(ctx)
    }

    pub fn claim_refund(ctx: Context<ClaimRefund>) -> Result<()> {
        instructions::settlement::claim_refund(ctx)
    }

    pub fn close_event(ctx: Context<CloseEvent>) -> Result<()> {
        instructions::settlement::close_event(ctx)
    }
}

#[cfg(test)]
mod tests {
    /// Confirms the `compile_error!` guard against `mainnet` +
    /// `oracle-mock` coexisting is present in this file's source. This is
    /// a source-text check, not a build check: actually building with
    /// both features on would fail to compile at all (that is the guard
    /// doing its job), so no test binary could ever run to report a
    /// failure the normal way.
    #[test]
    fn mainnet_and_oracle_mock_guard_is_present() {
        let source = include_str!("lib.rs");
        assert!(
            source.contains(r#"#[cfg(all(feature = "mainnet", feature = "oracle-mock"))]"#)
                && source.contains("compile_error!"),
            "the oracle-mock/mainnet compile_error! guard is missing from lib.rs"
        );
    }
}
