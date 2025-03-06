use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer, Mint};
// use solana_program::keccak::hash;

mod lock;

declare_id!("E87qEpEEcTk2bfN1CQqaC9qtm7zH7xCkSAm6qj6oaAcd");

// Define EXTRA_DECIMALS as a constant
const EXTRA_DECIMALS: u64 = 12; // Equivalent to 10^12
const RATE_LOCK_SELECTOR: &str = "RATE_LOCK";

#[program]
pub mod market_v {
    use super::*;

    // Initialize the market
    pub fn initialize(
        ctx: Context<Initialize>,
        selector: String,
        wait_time: u64,
        admin: Pubkey,
    ) -> Result<()> {
        let market = &mut ctx.accounts.market;

        // Set the admin authority
        market.admin = admin;

        // Set the token mint address
        market.token_mint = ctx.accounts.token_mint.key();
        // market.token_mint = *ctx.accounts.token_program.to_account_info().key;

        // Set the job index counter
        market.job_index = 0;

        // call update_lock_wait_time instruction to set the lock wait time
        let lock_wait_time = &mut ctx.accounts.lock_wait_time;
        // lock_wait_time.wait_time = wait_time;
        lock::update_lock_wait_time_util(lock_wait_time, selector, wait_time)?;

        Ok(())
    }

    // Add a provider
    pub fn provider_add(ctx: Context<ProviderAdd>, cp: String) -> Result<()> {
        let provider = &mut ctx.accounts.provider;

        // Check 1: Ensure the provider does not already exist
        require!(provider.cp.is_empty(), ErrorCodes::ProviderAlreadyExists);

        // Check 2: Ensure the control plane URL is not empty
        require!(!cp.is_empty(), ErrorCodes::InvalidControlPlaneUrl);

        // Set the control plane URL and authority
        provider.cp = cp;

        emit!(ProviderAdded {
            provider: *ctx.accounts.authority.key,
            cp: provider.cp.clone(),
        });

        Ok(())
    }

    // Remove a provider
    pub fn provider_remove(ctx: Context<ProviderRemove>) -> Result<()> {
        // Ensure the caller is the provider's authority
        let authority = ctx.accounts.authority.key();
        let (expected_provider_key, _) =
            Pubkey::find_program_address(&[b"provider", authority.as_ref()], ctx.program_id);

        require!(
            ctx.accounts.provider.key() == expected_provider_key,
            ErrorCodes::Unauthorized
        );

        // // Close the provider account
        // let provider_account = ctx.accounts.provider.to_account_info();
        // let authority = ctx.accounts.authority.to_account_info();

        // // Refund the rent to the authority
        // let rent = Rent::get()?;
        // let lamports = provider_account.lamports();
        // **provider_account.lamports.borrow_mut() = 0;
        // **authority.lamports.borrow_mut() += lamports;

        emit!(ProviderRemoved {
            provider: *ctx.accounts.authority.key,
        });

        Ok(())
    }

    // Update a provider's control plane URL
    pub fn provider_update_with_cp(
        ctx: Context<ProviderUpdateWithCp>,
        new_cp: String,
    ) -> Result<()> {
        let provider = &mut ctx.accounts.provider;

        // Check 1: Ensure the provider exists
        require!(!provider.cp.is_empty(), ErrorCodes::ProviderNotFound);

        // Check 2: Ensure the new control plane URL is not empty
        require!(!new_cp.is_empty(), ErrorCodes::InvalidControlPlaneUrl);

        // Update the control plane URL
        provider.cp = new_cp;

        emit!(ProviderUpdatedWithCp {
            provider: *ctx.accounts.authority.key,
            new_cp: provider.cp.clone(),
        });

        Ok(())
    }

    // Update the token mint address
    pub fn update_token(ctx: Context<UpdateToken>, new_token_mint: Pubkey) -> Result<()> {
        let market = &mut ctx.accounts.market;

        // // Emit the TokenUpdated event
        // emit!(TokenUpdated {
        //     old_token_mint: market.token_mint,
        //     new_token_mint,
        // });

        // // Update the token mint address
        // market.token_mint = new_token_mint;

        // Ok(())

        utils_mod::update_token_util(market, new_token_mint)?;

        Ok(())
    }

    // Open a new job
    pub fn job_open(
        ctx: Context<JobOpen>,
        metadata: String, // Changed to String
        provider: Pubkey,
        rate: u64,
        balance: u64,
    ) -> Result<()> {
        let market = &mut ctx.accounts.market;
        let job = &mut ctx.accounts.job;

        require_keys_eq!(ctx.accounts.token_mint.key(), market.token_mint, ErrorCodes::InvalidMint);

        // Transfer tokens from the owner to the job account
        let cpi_accounts = Transfer {
            from: ctx.accounts.owner_token_account.to_account_info(),
            to: ctx.accounts.job_token_account.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts
        );
        token::transfer(cpi_ctx, balance)?;

        // Increment the job index
        market.job_index += 1;

        // Initialize the job
        job.index = market.job_index;
        job.metadata = metadata; // Now a String
        job.owner = *ctx.accounts.owner.key;
        job.provider = provider;
        job.rate = rate;
        job.balance = balance;
        job.last_settled = Clock::get()?.unix_timestamp as u64;

        emit!(JobOpened {
            job: job.key(),
            metadata: job.metadata.clone(), // Cloning the String
            owner: job.owner,
            provider: job.provider,
            rate: job.rate,
            balance: job.balance,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    // Settle a job
    pub fn job_settle(ctx: Context<JobSettle>, job_index: u64) -> Result<()> {
        require_keys_eq!(ctx.accounts.token_mint.key(), ctx.accounts.market.token_mint, ErrorCodes::InvalidMint);

        let seeds: &[&[u8]] = &[b"job_token", &[ctx.bumps.job_token_account]];
        let signer_seeds: &[&[&[u8]]] = &[&seeds[..]];

        // Reuse the settle_job function
        utils_mod::settle_job(
            &mut ctx.accounts.job,
            &ctx.accounts.provider_token_account,
            &ctx.accounts.job_token_account,
            &ctx.accounts.market,
            &ctx.accounts.token_program,
            signer_seeds,
        )?;

        Ok(())
    }

    // Close a job
    pub fn job_close(ctx: Context<JobClose>, job_index: u64) -> Result<()> {
        let job = &mut ctx.accounts.job;

        // Ensure the caller is the owner of the job
        require!(
            job.owner == *ctx.accounts.owner.key,
            ErrorCodes::Unauthorized
        );

        require_keys_eq!(ctx.accounts.token_mint.key(), ctx.accounts.market.token_mint, ErrorCodes::InvalidMint);

        let seeds: &[&[u8]] = &[b"job_token", &[ctx.bumps.job_token_account]];
        let signer_seeds: &[&[&[u8]]] = &[&seeds[..]];

        // Reuse the settle_job function
        utils_mod::settle_job(
            job,
            &ctx.accounts.provider_token_account,
            &ctx.accounts.job_token_account,
            &ctx.accounts.market,
            &ctx.accounts.token_program,
            signer_seeds,

        )?;

        // Transfer remaining balance to the owner
        if job.balance > 0 {
            let owner_token_account = &ctx.accounts.owner_token_account;
            let cpi_accounts = Transfer {
                from: ctx.accounts.job_token_account.to_account_info(),
                to: owner_token_account.to_account_info(),
                authority: ctx.accounts.job_token_account.to_account_info(),
            };
            let cpi_ctx =
                CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
            token::transfer(cpi_ctx, job.balance)?;
        }

        // Close the job account and refund the rent to the owner
        // let job_account = job.to_account_info();
        // let owner = ctx.accounts.owner.to_account_info();

        // let rent = Rent::get()?;
        // let lamports = job_account.lamports();
        // **job_account.lamports.borrow_mut() = 0;
        // **owner.lamports.borrow_mut() += lamports;

        emit!(JobClosed { job: job.key() });

        Ok(())
    }

    // Deposit tokens into a job
    pub fn job_deposit(
        ctx: Context<JobDeposit>,
        job_index: u64, // Job index to identify the job
        amount: u64,    // Amount of tokens to deposit
    ) -> Result<()> {
        let job = &mut ctx.accounts.job;

        // Ensure the job exists
        require!(
            job.owner != Pubkey::default(),
            ErrorCodes::JobNotFound
        );

        require_keys_eq!(ctx.accounts.token_mint.key(), ctx.accounts.market.token_mint, ErrorCodes::InvalidMint);

        // Transfer tokens from the owner to the job's token account
        let cpi_accounts = Transfer {
            from: ctx.accounts.owner_token_account.to_account_info(),
            to: ctx.accounts.job_token_account.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        // Update the job's balance
        job.balance += amount;

        emit!(JobDeposited {
            job: job.key(),
            from: ctx.accounts.owner.key(),
            amount,
        });

        Ok(())
    }

    // Withdraw tokens from a job
    pub fn job_withdraw(
        ctx: Context<JobWithdraw>,
        job_index: u64, // Job index to identify the job
        amount: u64,    // Amount of tokens to withdraw
    ) -> Result<()> {
        let job = &mut ctx.accounts.job;

        // Ensure the job exists
        require!(
            job.owner != Pubkey::default(),
            ErrorCodes::JobNotFound
        );

        // Ensure the caller is the job owner
        require!(
            job.owner == *ctx.accounts.owner.key,
            ErrorCodes::Unauthorized
        );

        require_keys_eq!(ctx.accounts.token_mint.key(), ctx.accounts.market.token_mint, ErrorCodes::InvalidMint);

        // Ensure the job has sufficient balance
        require!(
            job.balance >= amount,
            ErrorCodes::InsufficientBalance
        );

        let seeds: &[&[u8]] = &[b"job_token", &[ctx.bumps.job_token_account]];
        let signer_seeds: &[&[&[u8]]] = &[&seeds[..]];
        // let signer_seeds: &[&[&[u8]]] = &[&[b"job_token", &[ctx.bumps.job_token_account]]];

        // Transfer tokens from the job's token account to the owner
        let cpi_accounts = Transfer {
            from: ctx.accounts.job_token_account.to_account_info(),
            to: ctx.accounts.owner_token_account.to_account_info(),
            authority: ctx.accounts.job_token_account.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(), cpi_accounts
        ).with_signer(signer_seeds);
        token::transfer(cpi_ctx, amount)?;

        // Update the job's balance
        job.balance -= amount;

        emit!(JobWithdrew {
            job: job.key(),
            to: ctx.accounts.owner.key(),
            amount,
        });

        Ok(())
    }

    // Initiate a rate revision for a job
    pub fn job_revise_rate_initiate(
        ctx: Context<JobReviseRateInitiate>,
        job_index: u64, // Job index to identify the job
        new_rate: u64,  // New rate to propose
    ) -> Result<()> {
        let selector = String::from(RATE_LOCK_SELECTOR);

        let job = &mut ctx.accounts.job;

        // Ensure the caller is the job owner
        require!(
            job.owner == *ctx.accounts.owner.key,
            ErrorCodes::Unauthorized
        );

        // Set the proposed rate
        job.rate = new_rate;

        // // Call the lock module's create_lock function
        // let cpi_accounts = lock::CreateLock {
        //     lock: ctx.accounts.lock,
        //     lock_wait_time: ctx.accounts.lock_wait_time,
        //     user: ctx.accounts.owner,
        //     system_program: ctx.accounts.system_program,
        // };
        // // let cpi_ctx = CpiContext::new(
        // //     ctx.accounts.system_program.to_account_info(),
        // //     cpi_accounts
        // // );
        // let cpi_ctx = Context::new(
        //     ctx.program_id,
        //     &mut cpi_accounts,
        //     &[],
        //     lock::CreateLockBumps {
        //         lock: *ctx.bumps.get("lock").unwrap(),
        //         lock_wait_time: *ctx.bumps.get("lock_wait_time").unwrap(),
        //     }
        // );
        // lock::lock_program::create_lock(cpi_ctx, selector, selector, new_rate)?;
        // lock::lock_program::create_lock(ctx, selector, selector, new_rate)?;
        lock::create_lock_util(
            &mut ctx.accounts.lock,
            ctx.accounts.lock_wait_time.wait_time,
            selector,
            job_index,
            new_rate
        )?;

        emit!(JobReviseRateInitiated {
            job: job.key(),
            new_rate,
        });

        Ok(())
    }

    pub fn job_revise_rate_cancel(
        ctx: Context<JobReviseRateCancel>,
        job_index: u64,
    ) -> Result<()> {
        let selector = String::from(RATE_LOCK_SELECTOR);
    
        // Ensure only the job owner can cancel
        require_keys_eq!(ctx.accounts.job.owner, ctx.accounts.user.key(), ErrorCodes::Unauthorized);
    
        // Call revert_lock in lock_program
        // let i_value = lock::lock_program::revert_lock(
        //     Context::new(
        //         ctx.program_id,
        //         lock::RevertLock {
        //             lock: ctx.accounts.lock.to_account_info(),
        //             user: ctx.accounts.user.to_account_info(),
        //         },
        //         ctx.remaining_accounts, // Pass any additional accounts
        //         lock::RevertLockBumps {
        //             lock: *ctx.bumps.get("lock").unwrap(),
        //         }
        //     ),
        //     selector,
        //     key
        // )?;
        lock::revert_lock_util(selector, job_index, ctx.accounts.lock.i_value)?;
    
        emit!(JobReviseRateCancelled {
            job: ctx.accounts.job.key(),
        });
    
        Ok(())
    }

    pub fn job_revise_rate_finalize(
        ctx: Context<JobReviseRateFinalize>,
        job_index: u64,
    ) -> Result<()> {
        let selector = String::from(RATE_LOCK_SELECTOR);
    
        // Ensure only the job owner can cancel
        require_keys_eq!(ctx.accounts.job.owner, ctx.accounts.user.key(), ErrorCodes::Unauthorized);
    
        // Call revert_lock in lock_program
        // let i_value = lock::lock_program::revert_lock(
        //     Context::new(
        //         ctx.program_id,
        //         lock::RevertLock {
        //             lock: ctx.accounts.lock.to_account_info(),
        //             user: ctx.accounts.user.to_account_info(),
        //         },
        //         ctx.remaining_accounts, // Pass any additional accounts
        //         lock::RevertLockBumps {
        //             lock: *ctx.bumps.get("lock").unwrap(),
        //         }
        //     ),
        //     selector,
        //     key
        // )?;
        let new_rate = lock::unlock_util(selector, job_index, ctx.accounts.lock.i_value, ctx.accounts.lock.unlock_time)?;
    
        emit!(JobReviseRateFinalized {
            job: ctx.accounts.job.key(),
            new_rate,
        });
    
        Ok(())
    }    

    mod utils_mod {
        use anchor_lang::accounts::signer;

        use super::*;

        pub fn update_token_util<'info>(
            market: &mut Account<'info, Market>,
            new_token_mint: Pubkey,
        ) -> Result<()> {
            // Emit the TokenUpdated event
            emit!(TokenUpdated {
                old_token_mint: market.token_mint,
                new_token_mint,
            });

            // Update the token mint address
            market.token_mint = new_token_mint;

            Ok(())
        }

        // Reusable function to settle a job
        pub(in crate::market_v) fn settle_job<'info>(
            job: &mut Account<'info, Job>,
            provider_token_account: &Account<'info, TokenAccount>,
            job_token_account: &Account<'info, TokenAccount>,
            market: &Account<'info, Market>,
            token_program: &Program<'info, Token>,
            signer_seeds: &[&[&[u8]]],
        ) -> Result<()> {
            // Calculate usage duration
            let usage_duration = Clock::get()?.unix_timestamp as u64 - job.last_settled;

            // Calculate amount to be paid
            let amount = (job.rate * usage_duration + 10u64.pow(EXTRA_DECIMALS as u32) - 1)
                / 10u64.pow(EXTRA_DECIMALS as u32);

            // Ensure the job has sufficient balance
            if amount > job.balance {
                job.balance = 0;
            } else {
                job.balance -= amount;
            }

            // Transfer tokens to the provider
            let cpi_accounts = Transfer {
                from: job_token_account.to_account_info(),
                to: provider_token_account.to_account_info(),
                authority: job_token_account.to_account_info(),
            };
            let cpi_ctx = CpiContext::new(
                token_program.to_account_info(), cpi_accounts
            ).with_signer(signer_seeds);
            token::transfer(cpi_ctx, amount)?;

            // Update the last settled timestamp
            job.last_settled = Clock::get()?.unix_timestamp as u64;

            emit!(JobSettled {
                job: job.key(),
                amount,
                timestamp: Clock::get()?.unix_timestamp,
            });

            Ok(())
        }

        // pub fn get_rate_lock_selector() -> [u8; 32] {
        //     let hash_result = hash(b"RATE_LOCK");
        //     hash_result.to_bytes()
        // }

    }

}

// Provider account
#[account]
pub struct Provider {
    pub cp: String,
}

// Market state
#[account]
pub struct Market {
    pub admin: Pubkey,      // Admin authority
    pub token_mint: Pubkey, // Token mint address
    pub job_index: u64,     // Job index counter
}

// Job account
#[account]
pub struct Job {
    pub index: u64,        // Job index
    pub metadata: String,  // Job metadata (now a String)
    pub owner: Pubkey,     // Job owner
    pub provider: Pubkey,  // Job provider
    pub rate: u64,         // Job rate
    pub balance: u64,      // Job balance
    pub last_settled: u64, // Last settled timestamp
}

// Contexts
#[derive(Accounts)]
#[instruction(selector: String)]
pub struct Initialize<'info> {
    #[account(init, payer = admin, space = 8 + std::mem::size_of::<Market>())]
    pub market: Account<'info, Market>,

    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = admin,
        seeds = [b"job_token", token_mint.key().as_ref()],
        bump,
        token::mint = token_mint,
        token::authority = job_token_account
    )]
    pub job_token_account: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = admin,
        // space = 8 + lock::LockWaitTime::INIT_SPACE,
        space = 8 + std::mem::size_of::<lock::LockWaitTime>(),
        // seeds = [b"lock_wait_time", selector.as_bytes().as_ref()],
        // seeds = [&wait_time.to_le_bytes()],
        seeds = [b"lock_wait_time", selector.as_bytes().as_ref()],
        // seeds = [b"lock_wait_time"],
        bump
    )]
    pub lock_wait_time: Account<'info, lock::LockWaitTime>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

// Context for adding a provider
#[derive(Accounts)]
pub struct ProviderAdd<'info> {
    // PDA for the provider account
    #[account(
        init,
        payer = authority,
        space = 8 + std::mem::size_of::<Provider>(),
        seeds = [b"provider", authority.key().as_ref()],
        bump
    )]
    pub provider: Account<'info, Provider>,

    // Authority (signer)
    #[account(mut)]
    pub authority: Signer<'info>,

    // System program
    pub system_program: Program<'info, System>,
}

// Context for removing a provider
#[derive(Accounts)]
pub struct ProviderRemove<'info> {
    // PDA for the provider account
    #[account(
        mut,
        close = authority,
        seeds = [b"provider", authority.key().as_ref()],
        bump
    )]
    pub provider: Account<'info, Provider>,

    // Authority (signer)
    #[account(mut)]
    pub authority: Signer<'info>,
}

// Context for updating a provider's control plane URL
#[derive(Accounts)]
pub struct ProviderUpdateWithCp<'info> {
    // PDA for the provider account
    #[account(
        mut,
        seeds = [b"provider", authority.key().as_ref()],
        bump
    )]
    pub provider: Account<'info, Provider>,

    // Authority (signer)
    #[account(mut)]
    pub authority: Signer<'info>,
}

// Context for updating the token mint address
#[derive(Accounts)]
pub struct UpdateToken<'info> {
    #[account(mut, has_one = admin @ ErrorCodes::Unauthorized)]
    pub market: Account<'info, Market>,

    #[account(mut)]
    pub admin: Signer<'info>,
}

// Context for opening a job
#[derive(Accounts)]
pub struct JobOpen<'info> {
    #[account(mut)]
    pub market: Account<'info, Market>,

    #[account(
        init,
        payer = owner,
        space = 8 + std::mem::size_of::<Job>(),
        seeds = [b"job", market.job_index.to_le_bytes().as_ref()], // Use job_index as seed
        bump
    )]
    pub job: Account<'info, Job>,

    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    #[account(mut)]
    pub owner_token_account: Account<'info, TokenAccount>,

    #[account(mut, seeds = [b"job_token", token_mint.key().as_ref()], bump)]
    pub job_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,

    pub system_program: Program<'info, System>,
}

// Context for settling a job
#[derive(Accounts)]
#[instruction(job_index: u64)]
pub struct JobSettle<'info> {
    #[account(
        mut,
        seeds = [b"job", job_index.to_le_bytes().as_ref()], // Use job_index as seed
        bump
    )]
    pub job: Account<'info, Job>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    #[account(mut, seeds = [b"job_token", token_mint.key().as_ref()], bump)]
    pub job_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub provider_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub market: Account<'info, Market>,

    pub token_program: Program<'info, Token>,
}

// Context for closing a job
#[derive(Accounts)]
#[instruction(job_index: u64)]
pub struct JobClose<'info> {
    #[account(
        mut,
        close = owner,
        seeds = [b"job", job_index.to_le_bytes().as_ref()], // Use job_index as seed
        bump
    )] // Close the job account and refund rent to the owner
    pub job: Account<'info, Job>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    #[account(mut, seeds = [b"job_token", token_mint.key().as_ref()], bump)]
    pub job_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub owner_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub provider_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub market: Account<'info, Market>,

    #[account(mut)]
    pub owner: Signer<'info>, // Owner must sign the transaction

    pub token_program: Program<'info, Token>,
}

// Context for depositing into a job
#[derive(Accounts)]
#[instruction(job_index: u64, amount: u64)]
pub struct JobDeposit<'info> {
    #[account(mut)]
    pub market: Account<'info, Market>,

    #[account(
        mut,
        seeds = [b"job", job_index.to_le_bytes().as_ref()], // Use job_index as seed
        bump
    )]
    pub job: Account<'info, Job>,

    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    #[account(mut)]
    pub owner_token_account: Account<'info, TokenAccount>,

    #[account(mut, seeds = [b"job_token", token_mint.key().as_ref()], bump)]
    pub job_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

// Context for withdrawing from a job
#[derive(Accounts)]
#[instruction(job_index: u64, amount: u64)]
pub struct JobWithdraw<'info> {
    #[account(
        mut,
        seeds = [b"job", job_index.to_le_bytes().as_ref()], // Use job_index as seed
        bump
    )]
    pub job: Account<'info, Job>,

    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    #[account(mut)]
    pub owner_token_account: Account<'info, TokenAccount>,

    #[account(mut, seeds = [b"job_token", token_mint.key().as_ref()], bump)]
    pub job_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub market: Account<'info, Market>,

    pub token_program: Program<'info, Token>,
}

// Context for initiating a rate revision
#[derive(Accounts)]
#[instruction(job_index: u64, new_rate: u64)]
pub struct JobReviseRateInitiate<'info> {
    #[account(
        mut,
        seeds = [b"job", job_index.to_le_bytes().as_ref()], // Use job_index as seed
        bump
    )]
    pub job: Account<'info, Job>,

    #[account(mut)]
    pub owner: Signer<'info>,

    // Accounts for the lock module's create_lock function
    #[account(
        init,
        payer = owner,
        space = 8 + lock::Lock::INIT_SPACE,
        seeds = [b"lock", RATE_LOCK_SELECTOR.as_bytes().as_ref(), job_index.to_le_bytes().as_ref()],
        bump
    )]
    pub lock: Account<'info, lock::Lock>,

    #[account(
        seeds = [b"lock_wait_time", RATE_LOCK_SELECTOR.as_bytes().as_ref()],
        bump
    )]
    pub lock_wait_time: Account<'info, lock::LockWaitTime>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(job_index: u64)]
pub struct JobReviseRateCancel<'info> {
    #[account(
        mut,
        seeds = [b"job", job_index.to_le_bytes().as_ref()], // Use job_index as seed
        bump
    )]
    pub job: Account<'info, Job>,

    #[account(
        mut,
        close = user,
        seeds = [b"lock", RATE_LOCK_SELECTOR.as_bytes().as_ref(), job_index.to_le_bytes().as_ref()],
        bump
    )]
    pub lock: Account<'info, lock::Lock>,

    #[account(mut)]
    pub user: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(job_index: u64)]
pub struct JobReviseRateFinalize<'info> {
    #[account(
        mut,
        seeds = [b"job", job_index.to_le_bytes().as_ref()], // Use job_index as seed
        bump
    )]
    pub job: Account<'info, Job>,

    #[account(
        mut,
        close = user,
        seeds = [b"lock", RATE_LOCK_SELECTOR.as_bytes().as_ref(), job_index.to_le_bytes().as_ref()],
        bump
    )]
    pub lock: Account<'info, lock::Lock>,

    #[account(mut)]
    pub user: Signer<'info>,
}

// Events
#[event]
pub struct ProviderAdded {
    pub provider: Pubkey,
    pub cp: String,
}

#[event]
pub struct ProviderRemoved {
    pub provider: Pubkey,
}

#[event]
pub struct ProviderUpdatedWithCp {
    pub provider: Pubkey,
    pub new_cp: String,
}

#[event]
pub struct TokenUpdated {
    pub old_token_mint: Pubkey,
    pub new_token_mint: Pubkey,
}

#[event]
pub struct JobOpened {
    pub job: Pubkey,
    pub metadata: String, // Now a String
    pub owner: Pubkey,
    pub provider: Pubkey,
    pub rate: u64,
    pub balance: u64,
    pub timestamp: i64,
}

#[event]
pub struct JobSettled {
    pub job: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct JobClosed {
    pub job: Pubkey,
}

#[event]
pub struct JobDeposited {
    pub job: Pubkey,
    pub from: Pubkey,
    pub amount: u64,
}

#[event]
pub struct JobWithdrew {
    pub job: Pubkey,
    pub to: Pubkey,
    pub amount: u64,
}

#[event]
pub struct JobReviseRateInitiated {
    pub job: Pubkey,
    pub new_rate: u64,
}

#[event]
pub struct JobReviseRateCancelled {
    pub job: Pubkey,
}

#[event]
pub struct JobReviseRateFinalized {
    pub job: Pubkey,
    pub new_rate: u64,
}

// Error codes
#[error_code]
pub enum ErrorCodes {
    #[msg("Provider already exists")]
    ProviderAlreadyExists,
    #[msg("Invalid control plane URL")]
    InvalidControlPlaneUrl,
    #[msg("Provider not found")]
    ProviderNotFound,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Job not found")]
    JobNotFound,
    #[msg("Insufficient balance")]
    InsufficientBalance,
    #[msg("Invalid mint")]
    InvalidMint,
}
