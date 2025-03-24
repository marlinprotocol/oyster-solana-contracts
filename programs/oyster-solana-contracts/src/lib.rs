use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer, Mint};
use oyster_credits::oyster_credits::redeem_and_burn;
use oyster_credits::RedeemAndBurn;
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
        notice_period: u64
    ) -> Result<()> {
        let market = &mut ctx.accounts.market;

        // Set the admin authority
        market.admin = admin;

        // Set the token mint address
        market.token_mint = ctx.accounts.token_mint.key();
        // market.token_mint = *ctx.accounts.token_program.to_account_info().key;

        // Set the job index counter
        market.job_index = 0;
        market.notice_period = notice_period;

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
        provider.owner = *ctx.accounts.authority.key;

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
    // #[inline(never)] // needed due to stack size violation
    pub fn job_open(
        ctx: Context<JobOpen>,
        metadata: String, // Changed to String
        provider: Pubkey,
        rate: u64,
        balance: u64,
    ) -> Result<()> {
        let market = &mut ctx.accounts.market;
        let job = &mut ctx.accounts.job;

        // require_keys_eq!(ctx.accounts.token_mint.key(), market.token_mint, ErrorCodes::InvalidMint);

        // // Transfer tokens from the owner to the job account
        // let cpi_accounts = Transfer {
        //     from: ctx.accounts.owner_token_account.to_account_info(),
        //     to: ctx.accounts.job_token_account.to_account_info(),
        //     authority: ctx.accounts.owner.to_account_info(),
        // };
        // let cpi_ctx = CpiContext::new(
        //     ctx.accounts.token_program.to_account_info(),
        //     cpi_accounts
        // );
        // token::transfer(cpi_ctx, balance)?;


        // Initialize the job
        job.index = market.job_index;
        job.metadata = metadata; // Now a String
        job.owner = *ctx.accounts.owner.key;
        job.provider = provider;
        // job.rate = rate;
        job.balance = balance;
        job.last_settled = Clock::get()?.unix_timestamp as u64;

        // Increment the job index
        market.job_index += 1;

        utils_mod::deposit_token(
            job,
            &mut ctx.accounts.credit_mint,
            &mut ctx.accounts.user_credit_token_account,
            &mut ctx.accounts.program_credit_token_account,
            &mut ctx.accounts.token_mint,
            &mut ctx.accounts.user_token_account,
            &mut ctx.accounts.program_token_account,
            &ctx.accounts.owner,
            &ctx.accounts.token_program,
            // job_index,
            balance
        )?;

        let token_mint_key = ctx.accounts.token_mint.key();
        let seeds: &[&[u8]] = &[b"job_token", token_mint_key.as_ref(), &[ctx.bumps.program_token_account]];
        let signer_seeds: &[&[&[u8]]] = &[&seeds[..]];

        utils_mod::job_revise_rate_internal(
            job,
            &ctx.accounts.token_mint,
            &mut ctx.accounts.program_token_account,
            &mut ctx.accounts.provider_token_account,
            &ctx.accounts.credit_mint,
            &mut ctx.accounts.program_credit_token_account,
            &ctx.accounts.token_program,
            signer_seeds,
            rate,
            market.notice_period
        )?;

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

        let token_mint_key = ctx.accounts.token_mint.key();
        let seeds: &[&[u8]] = &[b"job_token", token_mint_key.as_ref(), &[ctx.bumps.program_token_account]];
        let signer_seeds: &[&[&[u8]]] = &[&seeds[..]];

        // Reuse the settle_job function
        // utils_mod::settle_job(
        //     &mut ctx.accounts.job,
        //     &ctx.accounts.provider_token_account,
        //     &ctx.accounts.job_token_account,
        //     &ctx.accounts.market,
        //     &ctx.accounts.token_program,
        //     signer_seeds,
        // )?;
        let current_time = Clock::get()?.unix_timestamp as u64;

        let job_rate = ctx.accounts.job.rate;
        utils_mod::job_settle_internal(
            &mut ctx.accounts.job,
            job_rate,
            current_time,
            &ctx.accounts.token_program,
            &ctx.accounts.token_mint,
            &mut ctx.accounts.program_token_account,
            &mut ctx.accounts.provider_token_account,
            &ctx.accounts.credit_mint,
            &mut ctx.accounts.program_credit_token_account,
            signer_seeds
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

        let current_time = Clock::get()?.unix_timestamp as u64;
        let notice_period = ctx.accounts.market.notice_period;

        let token_mint_key = ctx.accounts.token_mint.key();
        let seeds: &[&[u8]] = &[b"job_token", token_mint_key.as_ref(), &[ctx.bumps.program_token_account]];
        let token_signer_seeds: &[&[&[u8]]] = &[&seeds[..]];

        utils_mod::job_settle_internal(
            job,
            job.rate,
            current_time + notice_period,
            &ctx.accounts.token_program,
            &ctx.accounts.token_mint,
            &mut ctx.accounts.program_token_account,
            &mut ctx.accounts.provider_token_account,
            &ctx.accounts.credit_mint,
            &mut ctx.accounts.program_credit_token_account,
            token_signer_seeds
        )?;

        // Close the job account and refund the rent to the owner
        // let job_account = job.to_account_info();
        // let owner = ctx.accounts.owner.to_account_info();

        // let rent = Rent::get()?;
        // let lamports = job_account.lamports();
        // **job_account.lamports.borrow_mut() = 0;
        // **owner.lamports.borrow_mut() += lamports;

        let balance = job.balance;
        if balance > 0 {
            let credit_mint_key = ctx.accounts.credit_mint.key();
            let seeds: &[&[u8]] = &[b"credit_token", credit_mint_key.as_ref(), &[ctx.bumps.program_credit_token_account]];
            let credit_signer_seeds: &[&[&[u8]]] = &[&seeds[..]];

            utils_mod::withdraw_internal(
                job,
                &ctx.accounts.token_mint,
                &mut ctx.accounts.program_token_account,
                &ctx.accounts.credit_mint,
                &mut ctx.accounts.program_credit_token_account,
                &mut ctx.accounts.user_credit_token_account,
                &ctx.accounts.token_program,
                &ctx.accounts.user_token_account,
                balance,
                token_signer_seeds,
                credit_signer_seeds
            )?;
        }

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
        require!(amount > 0, ErrorCodes::InvalidAmount);

        let current_time = Clock::get()?.unix_timestamp as u64;
        let notice_period = ctx.accounts.market.notice_period;

        let token_mint_key = ctx.accounts.token_mint.key();
        let seeds: &[&[u8]] = &[b"job_token", token_mint_key.as_ref(), &[ctx.bumps.program_token_account]];
        let signer_seeds: &[&[&[u8]]] = &[&seeds[..]];

        let res = utils_mod::job_settle_internal(
            job,
            job.rate,
            current_time + notice_period,
            &ctx.accounts.token_program,
            &ctx.accounts.token_mint,
            &mut ctx.accounts.program_token_account,
            &mut ctx.accounts.provider_token_account,
            &ctx.accounts.credit_mint,
            &mut ctx.accounts.program_credit_token_account,
            signer_seeds
        )?;
        require!(res, ErrorCodes::InsufficientFundsToReviseRate);

        utils_mod::deposit_token(
            job,
            &mut ctx.accounts.credit_mint,
            &mut ctx.accounts.user_credit_token_account,
            &mut ctx.accounts.program_credit_token_account,
            &mut ctx.accounts.token_mint,
            &mut ctx.accounts.user_token_account,
            &mut ctx.accounts.program_token_account,
            &ctx.accounts.owner,
            &ctx.accounts.token_program,
            amount
        )?;

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

        require!(amount > 0, ErrorCodes::InvalidAmount);

        let current_time = Clock::get()?.unix_timestamp as u64;
        let notice_period = ctx.accounts.market.notice_period;

        let token_mint_key = ctx.accounts.token_mint.key();
        let seeds: &[&[u8]] = &[b"job_token", token_mint_key.as_ref(), &[ctx.bumps.program_token_account]];
        let token_signer_seeds: &[&[&[u8]]] = &[&seeds[..]];

        let res = utils_mod::job_settle_internal(
            job,
            job.rate,
            current_time + notice_period,
            &ctx.accounts.token_program,
            &ctx.accounts.token_mint,
            &mut ctx.accounts.program_token_account,
            &mut ctx.accounts.provider_token_account,
            &ctx.accounts.credit_mint,
            &mut ctx.accounts.program_credit_token_account,
            token_signer_seeds
        )?;
        require!(res, ErrorCodes::InsufficientFundsToReviseRate);

        let credit_mint_key = ctx.accounts.credit_mint.key();
        let seeds: &[&[u8]] = &[b"credit_token", credit_mint_key.as_ref(), &[ctx.bumps.program_credit_token_account]];
        let credit_signer_seeds: &[&[&[u8]]] = &[&seeds[..]];

        utils_mod::withdraw_internal(
            job,
            &ctx.accounts.token_mint,
            &mut ctx.accounts.program_token_account,
            &ctx.accounts.credit_mint,
            &mut ctx.accounts.program_credit_token_account,
            &mut ctx.accounts.user_credit_token_account,
            &ctx.accounts.token_program,
            &ctx.accounts.user_token_account,
            amount,
            token_signer_seeds,
            credit_signer_seeds
        )?;

        Ok(())
    }

    pub fn job_revise_rate(
        ctx: Context<JobReviseRate>,
        job_index: u64, // Job index to identify the job
        new_rate: u64,  // New rate to propose
    ) -> Result<()> {
        let token_mint_key = ctx.accounts.token_mint.key();
        let seeds: &[&[u8]] = &[b"job_token", token_mint_key.as_ref(), &[ctx.bumps.program_token_account]];
        let signer_seeds: &[&[&[u8]]] = &[&seeds[..]];

        utils_mod::job_revise_rate_internal(
            &mut ctx.accounts.job,
            &ctx.accounts.token_mint,
            &mut ctx.accounts.program_token_account,
            &mut ctx.accounts.provider_token_account,
            &mut ctx.accounts.credit_mint,
            &mut ctx.accounts.program_credit_token_account,
            &ctx.accounts.token_program,
            signer_seeds,
            new_rate,
            ctx.accounts.market.notice_period
        )?;

        Ok(())
    }

    mod utils_mod {
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

        pub fn job_revise_rate_internal<'info>(
            job: &mut Account<'info, Job>,
            token_mint: &Account<'info, Mint>,
            program_token_account: &mut Account<'info, TokenAccount>,
            provider_token_account: &mut Account<'info, TokenAccount>,
            credit_mint: &Account<'info, Mint>,
            program_credit_token_account: &mut Account<'info, TokenAccount>,
            token_program: &Program<'info, Token>,
            signer_seeds: &[&[&[u8]]],
            new_rate: u64,
            notice_period: u64
        ) -> Result<()> {
            require!(new_rate > 0, ErrorCodes::InvalidRate);
            require!(job.rate != new_rate, ErrorCodes::UnchangedRate);

            let last_settled = job.last_settled;
            let current_time = Clock::get()?.unix_timestamp as u64;

            if current_time > last_settled {
                let res = job_settle_internal(
                    job,
                    job.rate,
                    current_time,
                    token_program,
                    token_mint,
                    program_token_account,
                    provider_token_account,
                    credit_mint,
                    program_credit_token_account,
                    signer_seeds
                )?;
                require!(res, ErrorCodes::InsufficientFundsToReviseRate);
            }

            let old_rate = job.rate;
            job.rate = new_rate;

            emit!(JobRateRevised {
                job: job.key(),
                new_rate,
            });

            let higher_rate = old_rate.max(new_rate);
            let res = job_settle_internal(
                job,
                higher_rate,
                current_time + notice_period,
                token_program,
                token_mint,
                program_token_account,
                provider_token_account,
                credit_mint,
                program_credit_token_account,
                signer_seeds
            )?;
            require!(res, ErrorCodes::InsufficientFundsToReviseRate);

            Ok(())
        }

        pub fn job_settle_internal<'info>(
            job: &mut Account<'info, Job>,
            rate: u64,
            settle_till: u64,
            token_program: &Program<'info, Token>,
            token_mint: &Account<'info, Mint>,
            program_token_account: &mut Account<'info, TokenAccount>,
            provider_token_account: &mut Account<'info, TokenAccount>,
            credit_mint: &Account<'info, Mint>,
            program_credit_token_account: &mut Account<'info, TokenAccount>,
            signer_seeds: &[&[&[u8]]],
        ) -> Result<bool> {
            let last_settled = job.last_settled;

            if settle_till == last_settled {
                return Ok(true);
            }
            require!(settle_till > last_settled, ErrorCodes::CannotSettle);

            let usage_duration = settle_till - last_settled;
            let amount_used = calculate_amount_used(rate, usage_duration);
            let settle_amount = amount_used.min(job.balance);

            msg!("SETTLE_TOKENS------");
            settle_tokens(
                job,
                token_mint,
                program_token_account,
                provider_token_account,
                credit_mint,
                program_credit_token_account,
                token_program,
                settle_amount,
                signer_seeds
            )?;

            job.last_settled = settle_till;

            emit!(JobSettled {
                job: job.key(),
                amount: settle_amount,
                timestamp: Clock::get()?.unix_timestamp,
            });

            Ok(amount_used <= settle_amount)
        }

        fn calculate_amount_used(rate: u64, usage_duration: u64) -> u64 {
            (rate * usage_duration + 10u64.pow(EXTRA_DECIMALS as u32) - 1) / 10u64.pow(EXTRA_DECIMALS as u32)
        }

        pub fn settle_tokens<'info>(
            job: &mut Account<'info, Job>,
            token_mint: &Account<'info, Mint>,
            program_token_account: &mut Account<'info, TokenAccount>,
            provider_token_account: &Account<'info, TokenAccount>,
            credit_mint: &Account<'info, Mint>,
            program_credit_token_account: &mut Account<'info, TokenAccount>,
            token_program: &Program<'info, Token>,
            amount: u64,
            signer_seeds: &[&[&[u8]]],
        ) -> Result<()> {
            // Deduct the amount from the job's balance
            job.balance -= amount;

            let mut token_amount = amount;
            let mut credit_amount = 0;

            if credit_mint.key() != Pubkey::default() {
                // Get the credit token balance
                let credit_balance = job.credit_balance;

                if credit_balance > 0 {
                    // Calculate the token split
                    (credit_amount, token_amount) = calculate_token_split(amount, credit_balance);

                    // Deduct the credit amount from the job's credit balance
                    job.credit_balance -= credit_amount;

                    // TODO: add cpi call to credit program
                    // // Transfer credit tokens to the provider
                    // let cpi_accounts = Transfer {
                    //     from: program_credit_token_account.to_account_info(),
                    //     to: provider_token_account.to_account_info(),
                    //     authority: program_credit_token_account.to_account_info(),
                    // };
                    // let cpi_ctx = CpiContext::new(token_program.to_account_info(), cpi_accounts);
                    // token::transfer(cpi_ctx, credit_amount)?;
                    // Perform a CPI call to the redeem_and_burn instruction in the oyster-credits program
                    // let cpi_program = credit_mint.to_account_info();
                    // let cpi_accounts = oyster_credits::RedeemAndBurn {
                    //     // state: program_credit_token_account.to_account_info(),
                    //     usdc_mint: *token_mint,
                    //     // program_usdc_token_account: *program_token_account,
                    //     user_usdc_token_account: *provider_token_account,
                    //     // credit_mint: credit_mint.to_account_info(),
                    //     // user_credit_token_account: program_credit_token_account.to_account_info(),
                    //     // token_program: token_program.to_account_info(),
                    //     // system_program: system_program.to_account_info(),
                    // };
                    // let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
                    // oyster_credits::cpi::redeem_and_burn(cpi_ctx, credit_amount)?;

                    emit!(JobSettlementWithdrawn {
                        job: job.key(),
                        token: credit_mint.key(),
                        provider: provider_token_account.owner,
                        amount: credit_amount,
                    });
                }
            }

            msg!("HEREEEE");
            if token_amount > 0 {
                msg!("TRANSFERRING...{:?}", token_amount);
                // Transfer tokens to the provider
                let cpi_accounts = Transfer {
                    from: program_token_account.to_account_info(),
                    to: provider_token_account.to_account_info(),
                    authority: program_token_account.to_account_info(),
                };
                let cpi_ctx = CpiContext::new(
                    token_program.to_account_info(),
                    cpi_accounts
                ).with_signer(signer_seeds);
                token::transfer(cpi_ctx, token_amount)?;

                emit!(JobSettlementWithdrawn {
                    job: job.key(),
                    token: program_token_account.mint,
                    provider: provider_token_account.owner,
                    amount: token_amount,
                });
                msg!("TRANSFER DONE!!!");
            }

            Ok(())
        }

        // pub fn get_rate_lock_selector() -> [u8; 32] {
        //     let hash_result = hash(b"RATE_LOCK");
        //     hash_result.to_bytes()
        // }

        pub fn deposit_token<'info>(
            job: &mut Account<'info, Job>,
            credit_mint: &mut Account<'info, Mint>,
            user_credit_token_account: &mut Account<'info, TokenAccount>,
            program_credit_token_account: &mut Account<'info, TokenAccount>,
            token_mint: &mut Account<'info, Mint>,
            user_token_account: &mut Account<'info, TokenAccount>,
            program_token_account: &mut Account<'info, TokenAccount>,
            signer: &Signer<'info>,
            token_program: &Program<'info, Token>,
            // job_index: u64,
            amount: u64
        ) -> Result<()> {
            let mut token_amount = amount;
            let mut credit_amount = 0;
    
            if credit_mint.key() != Pubkey::default() {
                // Get the credit token balance and allowance (TODO: check delegate)
                let credit_balance = user_credit_token_account.amount
                    .min(user_credit_token_account.delegated_amount);
    
                if credit_balance > 0 {
                    // Calculate the token split
                    (credit_amount, token_amount) = calculate_token_split(amount, credit_balance);
    
                    // Transfer credit tokens
                    let cpi_accounts = Transfer {
                        from: user_credit_token_account.to_account_info(),
                        to: program_credit_token_account.to_account_info(),
                        authority: signer.to_account_info(),
                    };
                    let cpi_ctx = CpiContext::new(
                        token_program.to_account_info(),
                        cpi_accounts
                    );
                    token::transfer(cpi_ctx, credit_amount)?;
    
                    // Update job credit balance
                    job.credit_balance += credit_amount;
    
                    emit!(JobDeposited {
                        job: job.key(),
                        from: signer.key(),
                        amount: credit_amount,
                    });
                }
            }
    
            if token_amount > 0 {
                // Transfer tokens
                let cpi_accounts = Transfer {
                    from: user_token_account.to_account_info(),
                    to: program_token_account.to_account_info(),
                    authority: signer.to_account_info(),
                };
                let cpi_ctx = CpiContext::new(
                    token_program.to_account_info(),
                    cpi_accounts
                );
                token::transfer(cpi_ctx, token_amount)?;
    
                emit!(JobDeposited {
                    job: job.key(),
                    from: signer.key(),
                    amount: token_amount,
                });
            }
    
            // Update job balance
            job.balance += amount;
    
            Ok(())
        }

        pub fn calculate_token_split(
            total_amount: u64,
            credit_balance: u64
        ) -> (u64, u64) {
            if total_amount > credit_balance {
                let credit_amount = credit_balance;
                let token_amount = total_amount - credit_balance;
                (credit_amount, token_amount)
            } else {
                let credit_amount = total_amount;
                let token_amount = 0;
                (credit_amount, token_amount)
            }
        }

        pub fn withdraw_internal<'info>(
            job: &mut Account<'info, Job>,
            token_mint: &Account<'info, Mint>,
            program_token_account: &mut Account<'info, TokenAccount>,
            credit_mint: &Account<'info, Mint>,
            program_credit_token_account: &mut Account<'info, TokenAccount>,
            user_credit_token_account: &mut Account<'info, TokenAccount>,
            token_program: &Program<'info, Token>,
            user_token_account: &Account<'info, TokenAccount>,
            amount: u64,
            token_signer_seeds: &[&[&[u8]]],
            credit_signer_seeds: &[&[&[u8]]],
        ) -> Result<()> {
            let job_balance = job.balance;
            require!(job_balance >= amount, ErrorCodes::InsufficientBalance);

            let mut withdraw_amount = amount;

            let job_credit_balance = job.credit_balance;
            require!(job_balance >= job_credit_balance, ErrorCodes::InvalidAmount);
            let job_token_balance = job_balance - job_credit_balance;

            job.balance -= withdraw_amount;

            let mut token_amount_to_transfer = 0;

            if job_token_balance < withdraw_amount {
                token_amount_to_transfer = job_token_balance;
                withdraw_amount -= job_token_balance;
            } else {
                token_amount_to_transfer = withdraw_amount;
                withdraw_amount = 0;
            }

            if token_amount_to_transfer > 0 {
                let cpi_accounts = Transfer {
                    from: program_token_account.to_account_info(),
                    to: user_token_account.to_account_info(),
                    authority: program_token_account.to_account_info(),
                };
                let cpi_ctx = CpiContext::new(
                    token_program.to_account_info(),
                    cpi_accounts,
                )
                .with_signer(token_signer_seeds);
                token::transfer(cpi_ctx, token_amount_to_transfer)?;

                emit!(JobWithdrew {
                    job: job.key(),
                    to: user_token_account.owner,
                    token: token_mint.key(),
                    amount: token_amount_to_transfer,
                });
            }

            if withdraw_amount > 0 {
                require!(credit_mint.key() != Pubkey::default(), ErrorCodes::InvalidMint);

                job.credit_balance -= withdraw_amount;

                let cpi_accounts = Transfer {
                    from: program_credit_token_account.to_account_info(),
                    to: user_credit_token_account.to_account_info(),
                    authority: program_credit_token_account.to_account_info(),
                };
                let cpi_ctx = CpiContext::new(
                    token_program.to_account_info(),
                    cpi_accounts,
                )
                .with_signer(credit_signer_seeds);
                token::transfer(cpi_ctx, withdraw_amount)?;

                emit!(JobWithdrew {
                    job: job.key(),
                    to: user_credit_token_account.owner,
                    token: credit_mint.key(),
                    amount: withdraw_amount,
                });
            }

            Ok(())
        }

    }

}

// Provider account
#[account]
#[derive(InitSpace)]
pub struct Provider {
    #[max_len(100)]
    pub cp: String,
    pub owner: Pubkey
}

// Market state
#[account]
pub struct Market {
    pub admin: Pubkey,      // Admin authority
    pub token_mint: Pubkey, // Token mint address
    pub credit_mint: Pubkey, // Credit mint address
    pub job_index: u64,     // Job index counter
    pub notice_period: u64
}

// Job account
#[account]
#[derive(InitSpace)]
pub struct Job {
    pub index: u64,             // Job index
    #[max_len(150)]
    pub metadata: String,       // Job metadata (now a String)
    pub owner: Pubkey,          // Job owner
    pub provider: Pubkey,       // Job provider
    pub rate: u64,              // Job rate
    pub balance: u64,           // Job balance
    pub last_settled: u64,      // Last settled timestamp
    pub credit_balance: u64,    // Credit balance
}

// Contexts
#[derive(Accounts)]
#[instruction(selector: String)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        seeds = [b"market"],
        bump,
        space = 8 + std::mem::size_of::<Market>()
    )]
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
        space = 8 + Provider::INIT_SPACE,
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
    #[account(
        mut,
        seeds = [b"market"],
        bump,
        has_one = admin @ ErrorCodes::Unauthorized
    )]
    pub market: Account<'info, Market>,

    #[account(mut)]
    pub admin: Signer<'info>,
}

// Context for opening a job
#[derive(Accounts)]
#[instruction(metadata: String, provider: Pubkey)]
pub struct JobOpen<'info> {
    #[account(
        mut,
        seeds = [b"market"],
        bump
    )]
    pub market: Box<Account<'info, Market>>,

    #[account(
        init,
        payer = owner,
        space = 8 + Job::INIT_SPACE,
        seeds = [b"job", market.job_index.to_le_bytes().as_ref()], // Use job_index as seed
        bump
    )]
    pub job: Box<Account<'info, Job>>,

    #[account(mut)]
    pub owner: Signer<'info>,

    // #[account(mut)]
    // pub token_mint: Account<'info, Mint>,

    // #[account(mut)]
    // pub owner_token_account: Account<'info, TokenAccount>,

    // #[account(mut, seeds = [b"job_token", token_mint.key().as_ref()], bump)]
    // pub job_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = token_mint.key() == market.token_mint
    )]
    pub token_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        seeds = [b"job_token", token_mint.key().as_ref()],
        bump,
        token::mint = token_mint,
        token::authority = program_token_account
    )]
    pub program_token_account: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub user_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = provider_token_account.owner == provider
    )]
    pub provider_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        // constraint = credit_mint.key() == market.credit_mint
    )]
    pub credit_mint: Box<Account<'info, Mint>>,

    #[account(
        init_if_needed,
        payer = owner,
        seeds = [b"credit_token", credit_mint.key().as_ref()],
        bump,
        token::mint = credit_mint,
        token::authority = program_credit_token_account
        // mut,
        // constraint = program_credit_token_account.owner == system_program.key()
    )]
    pub program_credit_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_credit_token_account.owner == owner.key()
    )]
    pub user_credit_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,

    pub system_program: Program<'info, System>,
}

// Context for settling a job
#[derive(Accounts)]
#[instruction(job_index: u64)]
pub struct JobSettle<'info> {
    #[account(
        mut,
        seeds = [b"market"],
        bump
    )]
    pub market: Account<'info, Market>,

    #[account(
        mut,
        seeds = [b"job", job_index.to_le_bytes().as_ref()], // Use job_index as seed
        bump,
        constraint = job.index == job_index
    )]
    pub job: Account<'info, Job>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    #[account(mut, seeds = [b"job_token", token_mint.key().as_ref()], bump)]
    pub program_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = provider_token_account.owner == job.provider,
    )]
    pub provider_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        // constraint = credit_mint.key() == market.credit_mint
    )]
    pub credit_mint: Account<'info, Mint>,

    #[account(
        init_if_needed,
        payer = owner,
        seeds = [b"credit_token", credit_mint.key().as_ref()],
        bump,
        token::mint = credit_mint,
        token::authority = program_credit_token_account
        // mut,
        // constraint = program_credit_token_account.owner == system_program.key()
    )]
    pub program_credit_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub owner: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

// Context for closing a job
#[derive(Accounts)]
#[instruction(job_index: u64)]
pub struct JobClose<'info> { 
    #[account(
        mut,
        seeds = [b"market"],
        bump
    )]
    pub market: Box<Account<'info, Market>>,

    #[account(
        mut,
        close = owner,
        seeds = [b"job", job_index.to_le_bytes().as_ref()], // Use job_index as seed
        bump,
        constraint = job.index == job_index
    )] // Close the job account and refund rent to the owner
    pub job: Box<Account<'info, Job>>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    #[account(mut, seeds = [b"job_token", token_mint.key().as_ref()], bump)]
    pub program_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_token_account.owner == job.owner,
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = provider_token_account.owner == job.provider
    )]
    pub provider_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        // constraint = credit_mint.key() == market.credit_mint
    )]
    pub credit_mint: Account<'info, Mint>,

    #[account(
        init_if_needed,
        payer = owner,
        seeds = [b"credit_token", credit_mint.key().as_ref()],
        bump,
        token::mint = credit_mint,
        token::authority = program_credit_token_account
        // mut,
        // constraint = program_credit_token_account.owner == system_program.key()
    )]
    pub program_credit_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_credit_token_account.owner == owner.key()
    )]
    pub user_credit_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub owner: Signer<'info>, // Owner must sign the transaction

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

// Context for depositing into a job
#[derive(Accounts)]
#[instruction(job_index: u64, amount: u64)]
pub struct JobDeposit<'info> {
    #[account(
        mut,
        seeds = [b"market"],
        bump
    )]
    pub market: Box<Account<'info, Market>>,

    #[account(
        mut,
        seeds = [b"job", job_index.to_le_bytes().as_ref()], // Use job_index as seed
        bump
    )]
    pub job: Box<Account<'info, Job>>,

    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut)]
    pub token_mint: Box<Account<'info, Mint>>,

    #[account(mut)]
    pub owner_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = provider_token_account.owner == job.provider,
    )]
    pub provider_token_account: Account<'info, TokenAccount>,

    #[account(mut, seeds = [b"job_token", token_mint.key().as_ref()], bump)]
    pub program_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        // constraint = credit_mint.key() == market.credit_mint
    )]
    pub credit_mint: Account<'info, Mint>,

    #[account(
        init_if_needed,
        payer = owner,
        seeds = [b"credit_token", credit_mint.key().as_ref()],
        bump,
        token::mint = credit_mint,
        token::authority = program_credit_token_account
        // mut,
        // constraint = program_credit_token_account.owner == system_program.key()
    )]
    pub program_credit_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_credit_token_account.owner == owner.key()
    )]
    pub user_credit_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

// Context for withdrawing from a job
#[derive(Accounts)]
#[instruction(job_index: u64, amount: u64)]
pub struct JobWithdraw<'info> {
    #[account(
        mut,
        seeds = [b"market"],
        bump
    )]
    pub market: Box<Account<'info, Market>>,

    #[account(
        mut,
        seeds = [b"job", job_index.to_le_bytes().as_ref()], // Use job_index as seed
        bump
    )]
    pub job: Box<Account<'info, Job>>,

    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut)]
    pub token_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = provider_token_account.owner == job.provider,
    )]
    pub provider_token_account: Account<'info, TokenAccount>,

    #[account(mut, seeds = [b"job_token", token_mint.key().as_ref()], bump)]
    pub program_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        // constraint = credit_mint.key() == market.credit_mint
    )]
    pub credit_mint: Account<'info, Mint>,

    #[account(
        init_if_needed,
        payer = owner,
        seeds = [b"credit_token", credit_mint.key().as_ref()],
        bump,
        token::mint = credit_mint,
        token::authority = program_credit_token_account
        // mut,
        // constraint = program_credit_token_account.owner == system_program.key()
    )]
    pub program_credit_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_credit_token_account.owner == owner.key()
    )]
    pub user_credit_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

// Context for a job rate revision
#[derive(Accounts)]
#[instruction(job_index: u64)]
pub struct JobReviseRate<'info> {
    #[account(
        mut,
        seeds = [b"market"],
        bump
    )]
    pub market: Box<Account<'info, Market>>,

    #[account(
        mut,
        seeds = [b"job", job_index.to_le_bytes().as_ref()], // Use job_index as seed
        bump
    )]
    pub job: Account<'info, Job>,

    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        constraint = token_mint.key() == market.token_mint
    )]
    pub token_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        seeds = [b"job_token", token_mint.key().as_ref()],
        bump,
        token::mint = token_mint,
        token::authority = program_token_account
    )]
    pub program_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = provider_token_account.owner == job.provider
    )]
    pub provider_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        // constraint = credit_mint.key() == market.credit_mint
    )]
    pub credit_mint: Box<Account<'info, Mint>>,

    #[account(
        init_if_needed,
        payer = owner,
        seeds = [b"credit_token", credit_mint.key().as_ref()],
        bump,
        token::mint = credit_mint,
        token::authority = program_credit_token_account
        // mut,
        // constraint = program_credit_token_account.owner == system_program.key()
    )]
    pub program_credit_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
// Events
#[event]
pub struct ProviderAdded {
    pub provider: Pubkey,
    pub cp: String,
}

#[event]
pub struct JobSettlementWithdrawn {
    pub job: Pubkey,
    pub token: Pubkey,
    pub provider: Pubkey,
    pub amount: u64,
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
    pub token: Pubkey,
    pub to: Pubkey,
    pub amount: u64,
}

#[event]
pub struct JobRateRevised {
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
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("Invalid Rate")]
    InvalidRate,
    #[msg("Rate is unchanged")]
    UnchangedRate,
    #[msg("Insufficient funds to revise rate")]
    InsufficientFundsToReviseRate,
    #[msg("Job not found")]
    JobNotFound,
    #[msg("Cannot settle before lastSettled")]
    CannotSettle,
    #[msg("Insufficient balance")]
    InsufficientBalance,
    #[msg("Invalid mint")]
    InvalidMint,
}
