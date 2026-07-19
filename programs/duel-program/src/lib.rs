//! Thin on-chain wrapper around the duel tick function — the replay target the
//! tickpruv referee CPIs into when a disputed tick is settled on-chain.
//!
//! Two-and-a-half instructions, selected by the first byte:
//!
//! 0 = Tick: tick index (u64 LE) then the raw price entry for that tick. The
//!     exact same `duel` crate runs off-chain in the runner; this instruction
//!     is what makes a disputed tick replayable by the chain itself.
//!
//! 1 = LoadState: raw state bytes copied verbatim into account 0. The referee
//!     seeds a throwaway scratch account with the agreed pre-state; the account
//!     must sign, so no one can overwrite a live match's state.
//!
//! 2 = Verdict: reads account 0 and publishes the match verdict (draw / agent A
//!     / agent B) as CPI return data, so the wager escrow can settle without
//!     knowing the state layout.

use duel::Duel;
use solana_program::{
    account_info::AccountInfo, entrypoint, entrypoint::ProgramResult, program::set_return_data,
    program_error::ProgramError, pubkey::Pubkey,
};
use tick_core::{TickError, TickLogic};

entrypoint!(process_instruction);

fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let state = accounts.first().ok_or(ProgramError::NotEnoughAccountKeys)?;
    if state.owner != program_id {
        return Err(ProgramError::IllegalOwner);
    }

    match data.split_first() {
        Some((0, rest)) => tick(state, rest),
        Some((1, rest)) => load_state(state, rest),
        Some((2, [])) => verdict(state),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn tick(state: &AccountInfo, data: &[u8]) -> ProgramResult {
    if data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let tick_index = u64::from_le_bytes(data[..8].try_into().unwrap());
    let inputs = &data[8..];

    let mut state_data = state.try_borrow_mut_data()?;
    Duel::tick(&mut state_data, inputs, tick_index).map_err(|e| match e {
        TickError::BadStateSize => ProgramError::InvalidAccountData,
        TickError::BadInput => ProgramError::InvalidInstructionData,
    })
}

fn verdict(state: &AccountInfo) -> ProgramResult {
    let state_data = state.try_borrow_data()?;
    let winner = duel::verdict(&state_data).map_err(|_| ProgramError::InvalidAccountData)?;
    set_return_data(&[winner]);
    Ok(())
}

fn load_state(state: &AccountInfo, data: &[u8]) -> ProgramResult {
    if !state.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    let mut state_data = state.try_borrow_mut_data()?;
    if state_data.len() != data.len() {
        return Err(ProgramError::InvalidAccountData);
    }
    state_data.copy_from_slice(data);
    Ok(())
}
