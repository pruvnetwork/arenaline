//! The duel: two autonomous trading agents step a deterministic strategy on
//! the same price stream and mark to market. A momentum agent chases the move;
//! a mean-reversion agent fades it. Whoever holds more equity at the final
//! settled price wins the match.
//!
//! This is a tickpruv game: the same crate compiles to SBF and to native, so a
//! disputed tick replays bit-identically on-chain via the referee. Inputs are
//! the sequenced price log (one price per tick); on-chain they live in account
//! data. Hard rules inherited from tick-core: no_std, no floats, no heap, all
//! math saturating so a hostile input can never open a panic path that behaves
//! differently off-chain than on-chain.

#![no_std]
#![deny(clippy::float_arithmetic)]
#![deny(unsafe_code)]

use tick_core::fx::{self, Fx};
use tick_core::{le, TickError, TickLogic};

/// State layout, little-endian:
///   0..8    tick counter (u64)
///   8..16   last price seen (Fx, Q32.32) — the momentum/reversion reference
///   16..24  agent A cash  (Fx)
///   24..32  agent A position, signed shares (Fx)
///   32..40  agent B cash  (Fx)
///   40..48  agent B position (Fx)
pub const STATE_SIZE: usize = 48;

/// One input entry: the market price for this tick (Fx, Q32.32). Price is an
/// implied probability in [0, 1] scaled by ONE — a share pays 1 if the outcome
/// lands, 0 if not, so buying at price `p` profits `1 - p` when it hits.
pub const INPUT_ENTRY_SIZE: usize = 8;

const T: usize = 0;
const LAST_PRICE: usize = 8;
const CASH_A: usize = 16;
const POS_A: usize = 24;
const CASH_B: usize = 32;
const POS_B: usize = 40;

/// Each agent opens with this much cash and no position.
pub const START_CASH: Fx = fx::from_int(100);
/// Shares traded per decision.
const UNIT: Fx = fx::ONE;
/// Position is bounded so equity stays in a sane, non-overflowing range.
const MAX_POS: Fx = fx::from_int(20);

/// Verdict encoding shared with the escrow program that settles on it.
pub mod side {
    pub const DRAW: u8 = 0;
    pub const FIRST: u8 = 1; // agent A (momentum)
    pub const SECOND: u8 = 2; // agent B (mean-reversion)
}

pub struct Duel;

impl Duel {
    /// Both agents start flat with equal cash; no price seen yet.
    pub fn init(state: &mut [u8]) -> Result<(), TickError> {
        if state.len() != STATE_SIZE {
            return Err(TickError::BadStateSize);
        }
        state.fill(0);
        le::write_i64(state, CASH_A, START_CASH);
        le::write_i64(state, CASH_B, START_CASH);
        Ok(())
    }
}

/// Apply `delta` shares at `price` for one agent, in place. Buying spends cash;
/// selling frees it. Position clamps to +/- MAX_POS; a buy that can't be paid
/// for is skipped, so cash never goes negative. All saturating.
fn trade(state: &mut [u8], cash_off: usize, pos_off: usize, delta: Fx, price: Fx) {
    let pos = le::read_i64(state, pos_off);
    let cash = le::read_i64(state, cash_off);
    let new_pos = pos.saturating_add(delta).clamp(-MAX_POS, MAX_POS);
    let filled = new_pos.saturating_sub(pos); // signed shares actually traded
    let cost = fx::mul(filled, price); // >0 when buying, <0 when selling
    if filled > 0 && cash < cost {
        return; // can't afford the buy — hold
    }
    le::write_i64(state, pos_off, new_pos);
    le::write_i64(state, cash_off, cash.saturating_sub(cost));
}

/// Mark-to-market equity for one agent at `price`, widened so the compare in
/// `verdict` cannot overflow even on adversarial state bytes.
fn equity(state: &[u8], cash_off: usize, pos_off: usize, price: Fx) -> i128 {
    let cash = le::read_i64(state, cash_off) as i128;
    let pos = le::read_i64(state, pos_off);
    cash + fx::mul(pos, price) as i128
}

/// Win condition over a final state: whoever holds more equity at the last
/// settled price wins. `price` at the final tick is the resolved outcome, so
/// equity is realized, not paper.
pub fn verdict(state: &[u8]) -> Result<u8, TickError> {
    if state.len() != STATE_SIZE {
        return Err(TickError::BadStateSize);
    }
    let price = le::read_i64(state, LAST_PRICE);
    let a = equity(state, CASH_A, POS_A, price);
    let b = equity(state, CASH_B, POS_B, price);
    Ok(match a.cmp(&b) {
        core::cmp::Ordering::Greater => side::FIRST,
        core::cmp::Ordering::Less => side::SECOND,
        core::cmp::Ordering::Equal => side::DRAW,
    })
}

/// Mark-to-market equity of both agents at the last price in `state`. For the
/// runner's live scoreboard; `verdict` is the on-chain-settled version.
pub fn equities(state: &[u8]) -> (i128, i128) {
    let price = le::read_i64(state, LAST_PRICE);
    (
        equity(state, CASH_A, POS_A, price),
        equity(state, CASH_B, POS_B, price),
    )
}

/// Current signed positions (agent A, agent B) in Fx shares.
pub fn positions(state: &[u8]) -> (Fx, Fx) {
    (le::read_i64(state, POS_A), le::read_i64(state, POS_B))
}

impl TickLogic for Duel {
    const STATE_SIZE: usize = STATE_SIZE;

    fn tick(state: &mut [u8], inputs: &[u8], tick_index: u64) -> Result<(), TickError> {
        if state.len() != STATE_SIZE {
            return Err(TickError::BadStateSize);
        }
        if inputs.len() != INPUT_ENTRY_SIZE {
            return Err(TickError::BadInput);
        }
        let price = le::read_i64(inputs, 0);
        let last = le::read_i64(state, LAST_PRICE);

        // Tick 0 only establishes the reference price — no trade before there
        // is a move to react to.
        if tick_index != 0 {
            // Momentum (A) chases the move; mean-reversion (B) fades it.
            let (da, db) = match price.cmp(&last) {
                core::cmp::Ordering::Greater => (UNIT, -UNIT),
                core::cmp::Ordering::Less => (-UNIT, UNIT),
                core::cmp::Ordering::Equal => (0, 0),
            };
            trade(state, CASH_A, POS_A, da, price);
            trade(state, CASH_B, POS_B, db, price);
        }

        le::write_i64(state, LAST_PRICE, price);
        let t = le::read_u64(state, T);
        le::write_u64(state, T, t.wrapping_add(1));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn price_entry(p: Fx) -> [u8; INPUT_ENTRY_SIZE] {
        let mut e = [0u8; INPUT_ENTRY_SIZE];
        e[0..8].copy_from_slice(&p.to_le_bytes());
        e
    }

    /// Drive the duel through a scripted price path and return the final state.
    fn run(prices: &[Fx]) -> [u8; STATE_SIZE] {
        let mut state = [0u8; STATE_SIZE];
        Duel::init(&mut state).unwrap();
        for (t, &p) in prices.iter().enumerate() {
            Duel::tick(&mut state, &price_entry(p), t as u64).unwrap();
        }
        state
    }

    // A steadily rising price: momentum (A) keeps buying into a market that
    // pays out higher, mean-reversion (B) keeps shorting into it. A must win.
    #[test]
    fn momentum_beats_reversion_on_a_trend() {
        let half = fx::ONE / 2;
        let step = fx::ONE / 50;
        let mut prices = [half; 30];
        for i in 1..prices.len() {
            prices[i] = prices[i - 1].saturating_add(step);
        }
        let state = run(&prices);
        assert_eq!(verdict(&state).unwrap(), side::FIRST);
    }

    // Deterministic: the same price path always lands on the same bytes. This
    // is the property the on-chain referee relies on for replay.
    #[test]
    fn identical_paths_are_bit_identical() {
        let half = fx::ONE / 2;
        let path: [Fx; 64] = core::array::from_fn(|i| {
            let wobble = if i % 2 == 0 { fx::ONE / 40 } else { -(fx::ONE / 40) };
            half.saturating_add(wobble)
        });
        assert_eq!(run(&path), run(&path));
    }

    // Cash is conserved as a bound: no agent can spend into the negative.
    #[test]
    fn no_agent_goes_cash_negative() {
        let half = fx::ONE / 2;
        let path: [Fx; 100] = core::array::from_fn(|i| {
            half.saturating_add(fx::mul(fx::from_int((i % 7) as i32 - 3), fx::ONE / 30))
        });
        let state = run(&path);
        assert!(le::read_i64(&state, CASH_A) >= 0, "A cash negative");
        assert!(le::read_i64(&state, CASH_B) >= 0, "B cash negative");
    }
}
