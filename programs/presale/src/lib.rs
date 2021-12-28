use anchor_lang::prelude::*;
use anchor_spl::token::{ self, Mint, MintTo, Burn, Token, TokenAccount, Transfer };

use std::ops::Deref;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

// USD coin's Decimal
const USDC_DECIMAL: u8 = 6;
// WEN token Decimal
// const WEN_DECIMAL:u8 = 9;
// NOTE: we need to consider decimals when we calculate all amount
// because decimals are different
// So we use DIVIDER for 10^6 / 10 ^9
const DIVIDER: u64 = 1000;

// Locked Rate
const LOCK_RATE: u64 = 50; // 50%
const DENOMINATOR: u64 = 100;

// Lock duration: 30 days
const LOCK_DURATION: i64 = 30 * 86400; // seconds

#[program]
pub mod presale {
    use super::*;

    // Initialize all infos of tokens and pool
    pub fn initialize(
        ctx: Context<Initialize>,
        presale_title: String,
        bumps: PoolBumps,
    ) -> ProgramResult {
        msg!("INITIALIZE POOL");
        
        let presale_account = &mut ctx.accounts.presale_account;

        let name_bytes = presale_title.as_bytes();
        let mut name_data = [b' '; 10];
        name_data[..name_bytes.len()].copy_from_slice(name_bytes);

        presale_account.presale_title = name_data;
        presale_account.bumps = bumps;

        presale_account.presale_owner = ctx.accounts.presale_owner.key();
        presale_account.usdc_mint = ctx.accounts.usdc_mint.key();
        presale_account.wen_mint = ctx.accounts.wen_mint.key();
        presale_account.pool_usdc = ctx.accounts.pool_usdc.key();
        presale_account.pool_wen = ctx.accounts.pool_wen.key();

        Ok(())
    }

    // Init user account
    pub fn init_user_account(
        ctx: Context<InitUserAccount>, 
        bump: u8, 
        seed0: String, 
        seed1: String
    ) -> ProgramResult {
        // Make as 1 string for pubkey
        let mut owned_string: String = seed0.to_owned();
        let another_owned_string: String = seed1.to_owned();
        owned_string.push_str(&another_owned_string);
        
        msg!("INIT USER INFO ACCOUNT {:?}, {:?}", owned_string, ctx.accounts.user_authority.key().to_string());
        if owned_string != ctx.accounts.user_authority.key().to_string() {
            return Err(ErrorCode::FailedInit.into())
        }

        let user_account = &mut ctx.accounts.user_account;
        user_account.owner = ctx.accounts.user_authority.key();
        user_account.bump = bump;
        Ok(())
    }

    // stake SEEDED token into player
    pub fn purchase(
        ctx: Context<DepositUsdcForWenToken>,
        amount: u64
    ) -> ProgramResult {
        msg!("Enter staking!!!");
        if amount < 1 {
            return Err(ErrorCode::InvalidAmount.into())
        }

        if ctx.accounts.user_usdc.amount < amount {
            return Err(ErrorCode::LowUSDC.into())
        }

        // Transfer user's USDC to pool USDC account.
        {
            let cpi_accounts = Transfer {
                // storer address (user address)
                from: ctx.accounts.user_usdc.to_account_info(),
                to: ctx.accounts.pool_usdc.to_account_info(),
                authority: ctx.accounts.user_authority.to_account_info(),
            };

            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
            token::transfer(cpi_ctx, amount)?;
        }

        let lock_amount = amount * LOCK_RATE / DENOMINATOR;
        let spend_amount = amount - lock_amount;

        // Transfer WEN token from pool token account to user's token account.
        {
            // Transfer seeded from pool account to the user's account.
            let presale_title = ctx.accounts.presale_account.presale_title.as_ref();
            let seeds = &[
                presale_title.trim_ascii_whitespace(),
                &[ctx.accounts.presale_account.bumps.presale_account],
            ];
            let signer = &[&seeds[..]];

            let cpi_accounts = Transfer {
                from: ctx.accounts.pool_wen.to_account_info(),
                to: ctx.accounts.user_wen.to_account_info(),
                authority: ctx.accounts.presale_account.to_account_info(),
            };
            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);

            token::transfer(cpi_ctx, spend_amount)?;
        }

        let clock = Clock::get()?; // Returns real-world time in second uint
        let user_account = &mut ctx.accounts.user_account;
        let deposit_amount = user_account.deposit_amount;
        let locked_amount = user_account.locked_amount;

        // Lock some tokens and will be able to claim after `LOCK_DURATION` days.
        user_account.locked_amount = locked_amount + lock_amount;
        user_account.deposit_amount = deposit_amount + amount;
        user_account.last_deposit_ts = clock.unix_timestamp;

        Ok(())
    }

    pub fn claim_locked_wen(
        ctx: Context<ClaimLockedWenToken>
    ) -> ProgramResult {        
        let user_account = &mut ctx.accounts.user_account;
        let locked_amount = user_account.locked_amount;
        let last_ts = user_account.last_deposit_ts;

        if locked_amount < 1 {
            return Err(ErrorCode::NotEnoughClaim.into())
        }

        let clock = Clock::get()?; // Returns real-world time in second uint
        let dur = last_ts - clock.unix_timestamp;
        if dur < LOCK_DURATION {
            return Err(ErrorCode::NotAbleClaim.into())
        }

        let spend_amount = locked_amount;
        if ctx.accounts.pool_wen.amount < spend_amount {
            return Err(ErrorCode::LowPoolWen.into())
        }

        // Transfer WEN token from pool token account to user's token account.
        {
            // Transfer seeded from pool account to the user's account.
            let presale_title = ctx.accounts.presale_account.presale_title.as_ref();
            let seeds = &[
                presale_title.trim_ascii_whitespace(),
                &[ctx.accounts.presale_account.bumps.presale_account],
            ];
            let signer = &[&seeds[..]];

            let cpi_accounts = Transfer {
                from: ctx.accounts.pool_wen.to_account_info(),
                to: ctx.accounts.user_wen.to_account_info(),
                authority: ctx.accounts.presale_account.to_account_info(),
            };
            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);

            token::transfer(cpi_ctx, spend_amount)?;
        }

        // update info
        user_account.locked_amount = 0;
        user_account.last_deposit_ts = clock.unix_timestamp;

        Ok(())

    }

    // After presale, admin can withdraw the remaining tokens
    pub fn withdraw_usdc(
        ctx: Context<WithdrawUSDC>
    ) -> ProgramResult {        
        let presale_account = &mut ctx.accounts.presale_account;
        

        if presale_account.presale_owner != user_authority {
            return Err(ErrorCode::NotRight.into())
        }

        let spend_amount = ctx.accounts.pool_usdc.amount;
        if spend_amount < 1 {
            return Err(ErrorCode::LowPoolUSDC.into())
        }

        // Transfer WEN token from pool token account to user's token account.
        {
            // Transfer seeded from pool account to the user's account.
            let presale_title = ctx.accounts.presale_account.presale_title.as_ref();
            let seeds = &[
                presale_title.trim_ascii_whitespace(),
                &[ctx.accounts.presale_account.bumps.presale_account],
            ];
            let signer = &[&seeds[..]];

            let cpi_accounts = Transfer {
                from: ctx.accounts.pool_usdc.to_account_info(),
                to: ctx.accounts.user_usdc.to_account_info(),
                authority: ctx.accounts.presale_account.to_account_info(),
            };
            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);

            token::transfer(cpi_ctx, spend_amount)?;
        }
        Ok(())
    }

    // After presale, admin can withdraw the remaining tokens
    pub fn withdraw_wen(
        ctx: Context<WithdrawWenToken>
    ) -> ProgramResult {        
        let presale_account = &mut ctx.accounts.presale_account;
        

        if presale_account.presale_owner != user_authority {
            return Err(ErrorCode::NotRight.into())
        }

        let spend_amount = ctx.accounts.pool_wen.amount;
        if spend_amount < 1 {
            return Err(ErrorCode::LowPoolWen.into())
        }

        // Transfer WEN token from pool token account to user's token account.
        {
            // Transfer seeded from pool account to the user's account.
            let presale_title = ctx.accounts.presale_account.presale_title.as_ref();
            let seeds = &[
                presale_title.trim_ascii_whitespace(),
                &[ctx.accounts.presale_account.bumps.presale_account],
            ];
            let signer = &[&seeds[..]];

            let cpi_accounts = Transfer {
                from: ctx.accounts.pool_wen.to_account_info(),
                to: ctx.accounts.user_wen.to_account_info(),
                authority: ctx.accounts.presale_account.to_account_info(),
            };
            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);

            token::transfer(cpi_ctx, spend_amount)?;
        }
        Ok(())
    }

    pub fn former_holders_config(
        ctx: Context<FormerHoldersList>,
        amount: u64
    ) -> ProgramResult {
        let presale_account = &mut ctx.accounts.presale_account;       

        if presale_account.presale_owner != user_authority {
            return Err(ErrorCode::NotRight.into())
        }

        let clock = Clock::get()?; // Returns real-world time in second uint

        let user_account = &mut ctx.accounts.user_account;
        user_account.locked_amount = amount;
        user_account.last_deposit_ts = clock.unix_timestamp;
    }
}

#[derive(Accounts)]
#[instruction(presale_title: String, bumps: PoolBumps)]
pub struct Initialize<'info> {
    // State Accounts
    #[account(
        init,
        seeds = [presale_title.as_bytes()],
        bump = bumps.presale_account,
        payer = user_authority
    )]
    pub presale_account: Account<'info, PresaleAccount>,
    // Contract Authority accounts
    #[account(mut)]
    pub presale_owner: Signer<'info>,
    // USDC Mint
    #[account(constraint = usdc_mint.decimals == USDC_DECIMAL)]
    pub usdc_mint: Account<'info, Mint>,
    // WEN token Mint
    #[account]
    pub wen_mint: Account<'info, Mint>,

    // USDC POOL
    #[account(
        init,
        token::mint = usdc_mint,
        token::authority = presale_account,
        seeds = [presale_title.as_bytes(), b"pool_usdc".as_ref()],
        bump = bumps.pool_usdc,
        payer = presale_owner
    )]
    pub pool_usdc: Account<'info, TokenAccount>,
    // WEN token POOL
    #[account(
        init,
        token::mint = wen_mint,
        token::authority = presale_account,
        seeds = [presale_title.as_bytes(), b"pool_wen".as_ref()],
        bump = bumps.pool_wen,
        payer = presale_owner
    )]
    pub pool_wen: Account<'info, TokenAccount>,

    // Programs and Sysvars
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>
}

#[derive(Accounts)]
#[instruction(bump: u8, seed0: String, seed1: String)]
pub struct InitUserAccount<'info> {
    // State account for each user/wallet
    #[account(
        init,
        seeds = [presale_account.presale_title.as_ref(), seed0.as_ref(), seed1.as_ref()],
        bump = bump,
        payer = user_authority
    )]
    pub user_account: Account<'info, UserInfoAccount>,
    pub presale_account: Account<'info, PresaleAccount>,
    // Contract Authority accounts
    #[account]
    pub user_authority: Signer<'info>,
    // Programs and Sysvars
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct DepositUsdcForWenToken<'info> {
    // Payable account (User wallet)
    #[account(mut)]
    pub user_authority: Signer<'info>,
    // User's info
    #[account(
        mut,
        constraint = user_account.owner == user_authority.key()
    )]
    pub user_account: Account<'info, UserInfoAccount>,
    // TODO replace these with the ATA constraints when possible. 
    // User's USDC token account
    #[account(
        constraint = user_usdc.owner ==user_authority.key(),
        constraint = user_usdc.mint == usdc_mint.key()
    )]
    pub user_usdc: Account<'info, TokenAccount>,
    // USD coin
    #[account(mut)]
    pub usdc_mint: Account<'info, Mint>,

    // User's WEN token account
    #[account(
        mut,
        constraint = user_wen.owner ==user_authority.key(),
        constraint = user_wen.mint == wen_mint.key()
    )]
    pub user_wen: Account<'info, TokenAccount>,
    // WEN token
    #[account(mut)]
    pub wen_mint: Account<'info, Mint>,
    
    #[account(
        seeds = [presale_account.presale_title.as_ref().trim_ascii_whitespace()],
        bump = presale_account.bumps.presale_account,
        has_one = usdc_mint
    )]
    pub presale_account: Box<Account<'info, PresaleAccount>>,
    
    // Pool for USDC and WEN
    #[account(
        seeds = [presale_account.presale_title.as_ref().trim_ascii_whitespace(), b"pool_usdc".as_ref()],
        bump = presale_account.bumps.pool_usdc)]
    pub pool_usdc: Account<'info, TokenAccount>,
    #[account(
        seeds = [presale_account.presale_title.as_ref().trim_ascii_whitespace(), b"pool_wen".as_ref()],
        bump = presale_account.bumps.pool_wen
    )]
    pub pool_wen: Account<'info, TokenAccount>,
    
    // Programs and Sysvars
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct ClaimLockedWenToken<'info> {
    // Payable account (User wallet)
    #[account]
    pub user_authority: Signer<'info>,
    // User's info
    #[account(
        mut,
        constraint = user_account.owner == user_authority.key()
    )]
    pub user_account: Account<'info, UserInfoAccount>,

    // User's WEN token account
    #[account(
        constraint = user_wen.owner ==user_authority.key(),
        constraint = user_wen.mint == wen_mint.key()
    )]
    pub user_wen: Account<'info, TokenAccount>,
    // WEN token
    #[account]
    pub wen_mint: Account<'info, Mint>,
    
    #[account(
        seeds = [presale_account.presale_title.as_ref().trim_ascii_whitespace(), b"pool_wen".as_ref()],
        bump = presale_account.bumps.pool_wen)]
    pub pool_wen: Account<'info, TokenAccount>,
    
    #[account(
        seeds = [presale_account.presale_title.as_ref().trim_ascii_whitespace()],
        bump = presale_account.bumps.presale_account,
        has_one = wen_mint)]
    pub presale_account: Box<Account<'info, PresaleAccount>>,
    
    // Programs and Sysvars
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct WithdrawUSDC<'info> {
    // Payable account (User wallet)
    #[account]
    pub user_authority: Signer<'info>,
    // TODO replace these with the ATA constraints when possible. 
    // User's USDC token account
    #[account(
        constraint = user_usdc.owner ==user_authority.key(),
        constraint = user_usdc.mint == usdc_mint.key()
    )]
    pub user_usdc: Account<'info, TokenAccount>,
    
    // Pool for USDC 
    #[account(
        seeds = [presale_account.presale_title.as_ref().trim_ascii_whitespace(), b"pool_usdc".as_ref()],
        bump = presale_account.bumps.pool_usdc
    )]
    pub pool_usdc: Account<'info, TokenAccount>,

    // USD coin
    #[account(mut)]
    pub usdc_mint: Account<'info, Mint>,
    
    #[account(
        seeds = [presale_account.presale_title.as_ref().trim_ascii_whitespace()],
        bump = presale_account.bumps.presale_account,
        has_one = usdc_mint
    )]
    pub presale_account: Box<Account<'info, PresaleAccount>>,
    
    // Programs and Sysvars
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}


// NOTE: we need to validate the owner
#[derive(Accounts)]
pub struct WithdrawWenToken<'info> {
    // Payable account (User wallet)
    #[account]
    pub user_authority: Signer<'info>,

    // User's WEN token account
    #[account(
        constraint = user_wen.owner ==user_authority.key(),
        constraint = user_wen.mint == wen_mint.key()
    )]
    pub user_wen: Account<'info, TokenAccount>,
    
    #[account(mut,
        seeds = [presale_account.presale_title.as_ref().trim_ascii_whitespace(), b"pool_wen".as_ref()],
        bump = presale_account.bumps.pool_wen)]
    pub pool_wen: Account<'info, TokenAccount>,

    // WEN token
    #[account(mut)]
    pub wen_mint: Account<'info, Mint>,
    
    #[account(
        seeds = [presale_account.presale_title.as_ref().trim_ascii_whitespace()],
        bump = presale_account.bumps.presale_account,
        has_one = wen_mint
    )]
    pub presale_account: Box<Account<'info, PresaleAccount>>,
    
    // Programs and Sysvars
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct FormerHoldersList<'info> {
    // Payable account (Owner wallet)
    #[account]
    pub user_authority: Signer<'info>,
    // User's info
    #[account(mut)]
    pub user_account: Account<'info, UserInfoAccount>,
    
    #[account(
        seeds = [presale_account.presale_title.as_ref().trim_ascii_whitespace()],
        bump = presale_account.bumps.presale_account,
        has_one = wen_mint)]
    pub presale_account: Box<Account<'info, PresaleAccount>>,
    
    // Programs and Sysvars
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[account]
#[derive(Default)]
pub struct PresaleAccount {
    pub presale_title: [u8; 10], // Setting an arbitrary max of ten characters in the presale name
    pub bumps: PoolBumps,
    pub presale_owner: Pubkey, // owner of program
    pub usdc_mint: Pubkey,
    pub wen_mint: Pubkey,
    pub pool_usdc: Pubkey,
    pub pool_wen: Pubkey
}

#[account]
#[derive(Default)]
pub struct UserInfoAccount {
    pub owner: Pubkey,
    pub bump: u8,
    pub deposit_amount: u64,
    pub locked_amount: u64,
    pub last_deposit_ts: i64
}

#[derive(AnchorSerialize, AnchorDeserialize, Default, Clone)]
pub struct PoolBumps {
    pub presale_account: u8,
    pub pool_usdc: u8,
    pub pool_wen: u8
}

#[error]
pub enum ErrorCode {
    #[msg("Insufficient USDC")]
    LowUSDC,
    #[msg("Insufficient wen tokens")]
    LowPoolWen,
    #[msg("USDC total and redeemable total don't match")]
    UsdcNotEqRedeem,
    #[msg("Initialize Stake Account Failed")]
    FailedInit,
    #[msg("Invalid amount to spend")]
    InvalidAmount,
    #[msg("Not enough to claim")]
    NotEnoughClaim,
    #[msg("Not able to claim")]
    NotAbleClaim,
    #[msg("You have no right to call this function")]
    NotRight,
    #[msg("Not enough USD coin")]
    LowPoolUSDC
}

/// Trait to allow trimming ascii whitespace from a &[u8].
pub trait TrimAsciiWhitespace {
    /// Trim ascii whitespace (based on `is_ascii_whitespace()`) from the
    /// start and end of a slice.
    fn trim_ascii_whitespace(&self) -> &[u8];
}

impl<T: Deref<Target = [u8]>> TrimAsciiWhitespace for T {
       
    fn trim_ascii_whitespace(&self) -> &[u8] {
        let from = match self.iter().position(|x| !x.is_ascii_whitespace()) {
            Some(i) => i,
            None => return &self[0..0],
        };
        let to = self.iter().rposition(|x| !x.is_ascii_whitespace()).unwrap();
        &self[from..=to]
    }
}
