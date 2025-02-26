use anchor_lang::prelude::*;

pub mod lock_program {
    use super::*;

    pub fn create_lock(
        ctx: Context<CreateLock>,
        selector: [u8; 32],
        key: [u8; 8],
        i_value: u64
    ) -> Result<()> {
        // let lock = &mut ctx.accounts.lock;
        // let clock = Clock::get()?.unix_timestamp as u64;
        // let wait_time = ctx.accounts.lock_wait_time.wait_time;
        // lock.unlock_time = clock + wait_time;
        // lock.i_value = i_value;

        // emit!(LockCreated {
        //     selector,
        //     key,
        //     i_value,
        //     unlock_time: lock.unlock_time
        // });
        create_lock_util(
            &mut ctx.accounts.lock,
            ctx.accounts.lock_wait_time.wait_time,
            selector,
            key,
            i_value
        )?;

        Ok(())
    }

    pub fn revert_lock(
        ctx: Context<RevertLock>,
        selector: [u8; 32],
        key: [u8; 8]
    ) -> Result<u64> {
        // let i_value = ctx.accounts.lock.i_value;
        // **ctx.accounts.lock.to_account_info().try_borrow_mut_lamports()? -= ctx.accounts.lock.to_account_info().lamports();

        // emit!(LockDeleted {
        //     selector,
        //     key,
        //     i_value
        // });

        let i_value = revert_lock_util(selector, key, ctx.accounts.lock.i_value)?;
        Ok(i_value)
    }

    pub fn unlock(
        ctx: Context<Unlock>,
        selector: [u8; 32],
        key: [u8; 8]
    ) -> Result<u64> {
        let i_value = unlock_util(selector, key, ctx.accounts.lock.i_value, ctx.accounts.lock.unlock_time)?;
        Ok(i_value)
    }

    pub fn clone_lock(
        ctx: Context<CloneLock>,
        selector: [u8; 32],
        from_key: [u8; 8],
        to_key: [u8; 8]
    ) -> Result<()> {
        let from_lock = &ctx.accounts.from_lock;
        let to_lock = &mut ctx.accounts.to_lock;
        to_lock.unlock_time = from_lock.unlock_time;
        to_lock.i_value = from_lock.i_value;

        emit!(LockCreated {
            selector: selector,
            key: to_key,
            i_value: to_lock.i_value,
            unlock_time: to_lock.unlock_time
        });

        Ok(())
    }

    pub fn update_lock_wait_time(
        ctx: Context<UpdateLockWaitTime>,
        selector: [u8; 32],
        new_wait_time: u64
    ) -> Result<()> {
        let lock_wait_time = &mut ctx.accounts.lock_wait_time;
        // emit!(LockWaitTimeUpdated {
        //     selector: selector,
        //     prev_lock_time: lock_wait_time.wait_time,
        //     updated_lock_time: new_wait_time
        // });

        // lock_wait_time.wait_time = new_wait_time;

        update_lock_wait_time_util(lock_wait_time, selector, new_wait_time)?;

        Ok(())
    }
}

pub fn create_lock_util(
    lock: &mut Account<'_, Lock>,
    wait_time: u64,
    selector: [u8; 32],
    key: [u8; 8],
    i_value: u64
) -> Result<()> {
    require!(lock.unlock_time == 0, ErrorCode::LockAlreadyExists);

    let clock = Clock::get()?.unix_timestamp as u64;
    lock.unlock_time = clock + wait_time;
    lock.i_value = i_value;

    emit!(LockCreated {
        selector,
        key,
        i_value,
        unlock_time: lock.unlock_time
    });

    Ok(())
}

pub fn revert_lock_util(
    selector: [u8; 32],
    key: [u8; 8],
    i_value: u64
) -> Result<u64> {
    emit!(LockDeleted {
        selector,
        key,
        i_value
    });

    Ok(i_value)
}

pub fn unlock_util(
    selector: [u8; 32],
    key: [u8; 8],
    i_value: u64,
    unlock_time: u64,
) -> Result<u64> {
    let clock = Clock::get()?.unix_timestamp as u64;
    require!(clock >= unlock_time, ErrorCode::LockNotYetUnlocked);

    revert_lock_util(selector, key, i_value)?;

    Ok(i_value)
}

pub fn update_lock_wait_time_util(
    lock_wait_time: &mut Account<'_, LockWaitTime>,
    selector: [u8; 32],
    new_wait_time: u64
) -> Result<()> {
    emit!(LockWaitTimeUpdated {
        selector: selector,
        prev_lock_time: lock_wait_time.wait_time,
        updated_lock_time: new_wait_time
    });

    lock_wait_time.wait_time = new_wait_time;

    Ok(())
}

#[derive(Accounts)]
#[instruction(selector: [u8; 32], key: [u8; 8])]
pub struct CreateLock<'info> {
    #[account(
        init,
        payer = user,
        space = 8 + Lock::INIT_SPACE,
        seeds = [b"lock", selector.as_ref(), key.as_ref()],
        bump
    )]
    pub lock: Account<'info, Lock>,

    #[account(
        seeds = [b"lock_wait_time", selector.as_ref()],
        bump
    )]
    pub lock_wait_time: Account<'info, LockWaitTime>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(selector: [u8; 32], key: [u8; 8])]
pub struct RevertLock<'info> {
    #[account(
        mut,
        close = user,
        seeds = [b"lock", selector.as_ref(), key.as_ref()],
        bump
    )]
    pub lock: Account<'info, Lock>,
    #[account(mut)]
    pub user: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(selector: [u8; 32], key: [u8; 8])]
pub struct Unlock<'info> {
    #[account(
        mut,
        close = user,
        seeds = [b"lock", selector.as_ref(), key.as_ref()],
        bump
    )]
    pub lock: Account<'info, Lock>,
    #[account(mut)]
    pub user: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(selector: [u8; 32], from_key: [u8; 8], to_key: [u8; 8])]
pub struct CloneLock<'info> {
    #[account(
        seeds = [b"lock", selector.as_ref(), from_key.as_ref()],
        bump
    )]
    pub from_lock: Account<'info, Lock>,
    #[account(
        init,
        payer = user,
        space = 8 + Lock::INIT_SPACE,
        seeds = [b"lock", selector.as_ref(), to_key.as_ref()],
        bump
    )]
    pub to_lock: Account<'info, Lock>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(selector: [u8; 32])]
pub struct UpdateLockWaitTime<'info> {
    #[account(
        init_if_needed,
        payer = user,
        space = 8 + LockWaitTime::INIT_SPACE,
        seeds = [b"lock_wait_time", selector.as_ref()],
        bump
    )]
    pub lock_wait_time: Account<'info, LockWaitTime>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}


#[account]
#[derive(InitSpace)]
pub struct Lock {
    pub unlock_time: u64,
    pub i_value: u64,
}

#[account]
#[derive(InitSpace)]
pub struct LockWaitTime {
    pub wait_time: u64,
}


#[event]
pub struct LockWaitTimeUpdated {
    pub selector: [u8; 32],
    pub prev_lock_time: u64,
    pub updated_lock_time: u64,
}

#[event]
pub struct LockCreated {
    pub selector: [u8; 32],
    pub key: [u8; 8],
    pub i_value: u64,
    pub unlock_time: u64,
}

#[event]
pub struct LockDeleted {
    pub selector: [u8; 32],
    pub key: [u8; 8],
    pub i_value: u64,
}

// Error codes
#[error_code]
pub enum ErrorCode {
    #[msg("Lock already exists")]
    LockAlreadyExists,
    #[msg("Lock not yet unlocked")]
    LockNotYetUnlocked,
}
