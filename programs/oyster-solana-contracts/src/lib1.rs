use anchor_lang::prelude::*;

// declare_id!("7iLD32W3pNCBX8op3V3Zv1KC5j5aRXkpPhVhWRPjXWGX");

// #[program]
pub mod oyster_solana_contracts {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
