use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer, Burn, MintTo};

declare_id!("GSFiwh4mAcbaD2yCU1jZh8YxLLzCkM4snWPQaBtS6GvY");

#[program]
pub mod oyster_credits {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        admin: Pubkey,
        oyster_market: Pubkey,
        usdc_mint: Pubkey
    ) -> Result<()> {
        let state = &mut ctx.accounts.state;
        require!(!state.initialized, ErrorCodes::AlreadyInitialized);

        state.admin = admin;
        state.oyster_market = oyster_market;
        state.usdc_mint = usdc_mint;
        state.initialized = true;
        Ok(())
    }

    pub fn mint(ctx: Context<MintTokens>, amount: u64) -> Result<()> {
        require!(ctx.accounts.state.admin == *ctx.accounts.signer.key, ErrorCodes::OnlyAdmin);

        let signer_seeds: &[&[&[u8]]] = &[&[b"credit_mint", &[ctx.bumps.credit_mint]]];

        let cpi_accounts = MintTo {
            mint: ctx.accounts.credit_mint.to_account_info(),
            to: ctx.accounts.token_account.to_account_info(),
            authority: ctx.accounts.credit_mint.to_account_info(),
        };

        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts
        ).with_signer(signer_seeds);
        token::mint_to(cpi_ctx, amount)?;

        Ok(())
    }

    pub fn burn(ctx: Context<BurnTokens>, amount: u64) -> Result<()> {
        require!(ctx.accounts.state.admin == *ctx.accounts.authority.key, ErrorCodes::OnlyAdmin);

        let signer_seeds: &[&[&[u8]]] = &[&[b"credit_mint", &[ctx.bumps.credit_mint]]];

        let cpi_accounts = Burn {
            mint: ctx.accounts.credit_mint.to_account_info(),
            from: ctx.accounts.token_account.to_account_info(),
            authority: ctx.accounts.authority.to_account_info(),
        };

        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts
        ).with_signer(signer_seeds);
        token::burn(cpi_ctx, amount)?;

        Ok(())
    }

    pub fn redeem_and_burn(ctx: Context<RedeemAndBurn>, amount: u64) -> Result<()> {
        // require!(!ctx.accounts.state.paused, ErrorCode::ContractPaused);

        let usdc_balance = ctx.accounts.program_usdc_token_account.amount;
        require!(usdc_balance >= amount, ErrorCodes::NotEnoughUSDC);

        // transfer usdc from the program to user
        let usdc_mint: Pubkey = ctx.accounts.usdc_mint.key();
        let signer_seeds: &[&[&[u8]]] = &[&[
            b"program_usdc", usdc_mint.as_ref(), &[ctx.bumps.program_usdc_token_account]
        ]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.program_usdc_token_account.to_account_info(),
            to: ctx.accounts.user_usdc_token_account.to_account_info(),
            authority: ctx.accounts.program_usdc_token_account.to_account_info(),
        };

        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts
        ).with_signer(signer_seeds);
        token::transfer(cpi_ctx, amount)?;

        // burn credits of the market program
        let cpi_accounts_burn = Burn {
            mint: ctx.accounts.credit_mint.to_account_info(),
            from: ctx.accounts.market_program_credit_token_account.to_account_info(),
            authority: ctx.accounts.credit_mint.to_account_info(),
        };

        let cpi_ctx_burn = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts_burn
        );
        token::burn(cpi_ctx_burn, amount)?;

        Ok(())
    }

    // pub fn emergency_withdraw(ctx: Context<EmergencyWithdraw>, amount: u64) -> Result<()> {
    //     require!(ctx.accounts.state.admin == *ctx.accounts.authority.key, ErrorCodes::OnlyAdmin);

    //     let cpi_accounts = Transfer {
    //         from: ctx.accounts.token_account.to_account_info(),
    //         to: ctx.accounts.receiver.to_account_info(),
    //         authority: ctx.accounts.state.to_account_info(),
    //     };

    //     let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
    //     token::transfer(cpi_ctx, amount)?;

    //     Ok(())
    // }

    // pub fn pause(ctx: Context<AdminAction>) -> Result<()> {
    //     ctx.accounts.state.paused = true;
    //     Ok(())
    // }

    // pub fn unpause(ctx: Context<AdminAction>) -> Result<()> {
    //     ctx.accounts.state.paused = false;
    //     Ok(())
    // }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = signer,
        space = 8 + State::INIT_SPACE,
        seeds = [b"state"],
        bump
    )]
    pub state: Account<'info, State>,

    #[account(
        init,
        payer = signer,
        seeds = [b"credit_mint"],
        bump,
        mint::decimals = 6,
        mint::authority = credit_mint,
    )]
    pub credit_mint: Account<'info, Mint>,

    #[account(mut)]
    pub signer: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MintTokens<'info> {
    #[account(
        mut,
        seeds = [b"state"],
        bump
    )]
    pub state: Account<'info, State>,

    #[account(
        mut,
        seeds = [b"credit_mint"],
        bump
    )]
    pub credit_mint: Account<'info, Mint>,

    #[account(mut)]
    pub token_account: Account<'info, TokenAccount>,
    pub signer: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct BurnTokens<'info> {
    #[account(
        mut,
        seeds = [b"state"],
        bump
    )]
    pub state: Account<'info, State>,

    #[account(
        mut,
        seeds = [b"credit_mint"],
        bump
    )]
    pub credit_mint: Account<'info, Mint>,

    #[account(mut)]
    pub token_account: Account<'info, TokenAccount>,
    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct RedeemAndBurn<'info> {
    #[account(
        mut,
        seeds = [b"state"],
        bump
    )]
    pub state: Account<'info, State>,

    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mut,
        constraint = usdc_mint.key() == state.usdc_mint
    )]
    pub usdc_mint: Account<'info, Mint>,

    #[account(
        init_if_needed,
        payer = signer,
        seeds = [b"program_usdc", usdc_mint.key().as_ref()],
        bump,
        token::mint = usdc_mint,
        token::authority = program_usdc_token_account
    )]
    pub program_usdc_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_usdc_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"credit_mint"],
        bump
    )]
    pub credit_mint: Account<'info, Mint>,

    #[account(
        mut,
        seeds = [b"credit_token", credit_mint.key().as_ref()],
        bump,
        token::mint = credit_mint,
        token::authority = market_program_credit_token_account,
        seeds::program = state.oyster_market
        // mut,
        // constraint = program_credit_token_account.owner == system_program.key()
    )]
    pub market_program_credit_token_account: Account<'info, TokenAccount>,

    // #[account(mut)]
    // pub user_credit_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct EmergencyWithdraw<'info> {
    #[account(mut)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    /// CHECK: ?
    pub receiver: AccountInfo<'info>,
    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct AdminAction<'info> {
    #[account(mut)]
    pub state: Account<'info, State>,
    pub authority: Signer<'info>,
}

#[account]
#[derive(InitSpace)]
pub struct State {
    pub admin: Pubkey,
    pub oyster_market: Pubkey,
    pub usdc_mint: Pubkey,
    pub initialized: bool,
}

#[error_code]
pub enum ErrorCodes {
    #[msg("Already initialized")]
    AlreadyInitialized,
    #[msg("Only admin can perform this action.")]
    OnlyAdmin,
    #[msg("Contract is currently paused.")]
    ContractPaused,
    #[msg("Not enough USDC balance.")]
    NotEnoughUSDC,
}
