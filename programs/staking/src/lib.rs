use anchor_lang::prelude::*;
use anchor_spl::token::{self, CloseAccount, Mint, Token, SetAuthority, MintTo, TokenAccount, Transfer};
use spl_token::instruction::AuthorityType;

declare_id!("FvSyVqCPvwVJ8XyNVLb3h4vUV36nr8c26CpF6YyL8zUu");


const VAULT_PDA_SEED: &[u8] = b"vault";
const STAKING_AMOUNT: u64 = 1;

const TIER_ONE_REWARD_RATE: u64 = 5;
const TIER_TWO_REWARD_RATE: u64 = 10;
const TIER_ONE_THRESHHOLD: i64 = 5;

const MINIMUM_STAKING_PERIOD: i64 = 2;
const MINIMUM_COLLECTION_PERIOD: i64 = 1;

#[program]
pub mod staking {
    use super::*;
    pub fn stake(ctx: Context<Stake>, _vault_account_bump: u8) -> ProgramResult {

        let clock: Clock = Clock::get().unwrap();
        let timestamp = clock.unix_timestamp;
        let earliest_unstake = timestamp + MINIMUM_STAKING_PERIOD;

        ctx.accounts.staking_account.staking_token_owner = *ctx.accounts.staking_token_owner.key;
        ctx.accounts
            .staking_account
            .owner_staking_token_account = *ctx
            .accounts
            .owner_staking_token_account
            .to_account_info()
            .key;
        ctx.accounts.staking_account.staking_mint = *ctx.accounts.staking_mint.to_account_info().key;
        
        ctx.accounts.staking_account.created = timestamp;
        ctx.accounts.staking_account.last_reward_collection = timestamp;
        ctx.accounts.staking_account.total_reward_collected = 0;
        ctx.accounts.staking_account.earliest_unstake = earliest_unstake;

        let (vault_authority, _vault_authority_bump) =
            Pubkey::find_program_address(&[VAULT_PDA_SEED, &*ctx.accounts.staking_account.to_account_info().key.as_ref(), &*ctx.accounts.staking_mint.to_account_info().key.as_ref()], ctx.program_id);

        token::set_authority(
            ctx.accounts.into_set_authority_context(),
            AuthorityType::AccountOwner,
            Some(vault_authority),
        )?;

        token::transfer(
            ctx.accounts.into_transfer_to_pda_context(),
            STAKING_AMOUNT,
        )?;

        Ok(())
    }

    pub fn collect(ctx: Context<Collect>, duration: u64) -> ProgramResult {

        let clock: Clock = Clock::get().unwrap();
        let timestamp = clock.unix_timestamp;
        let elapsed: i64 = timestamp - ctx.accounts.staking_account.last_reward_collection;

        if elapsed < MINIMUM_COLLECTION_PERIOD {
             return Err(ErrorCode::CollectionPeriodTooShort.into())
        }

        let mut amount: u64 = 0;

        if elapsed < TIER_ONE_THRESHHOLD {
            amount = TIER_ONE_REWARD_RATE * duration;
        }

        if elapsed > TIER_ONE_THRESHHOLD {
            amount = TIER_TWO_REWARD_RATE * duration;
        }

        token::mint_to(ctx.accounts.into_mint_to_staker(), amount).unwrap();

        ctx.accounts.staking_account.last_reward_collection = timestamp;
        ctx.accounts.staking_account.total_reward_collected = ctx.accounts.staking_account.total_reward_collected + amount;

        Ok(())
    }


    pub fn unstake(ctx: Context<Unstake>) -> ProgramResult {

        let clock: Clock = Clock::get().unwrap();
        let timestamp = clock.unix_timestamp;
        let elapsed: i64 = timestamp - ctx.accounts.staking_account.created;

        if elapsed < MINIMUM_STAKING_PERIOD {
             return Err(ErrorCode::StakingPeriodTooShort.into())
        }

        let (_vault_authority, vault_authority_bump) =
            Pubkey::find_program_address(&[VAULT_PDA_SEED, &*ctx.accounts.staking_account.to_account_info().key.as_ref(), &*ctx.accounts.staking_mint.to_account_info().key.as_ref()], ctx.program_id);

        let authority_seeds = &[&VAULT_PDA_SEED, &*ctx.accounts.staking_account.to_account_info().key.as_ref(), &*ctx.accounts.staking_mint.to_account_info().key.as_ref(), &[vault_authority_bump]];

        token::transfer(
            ctx.accounts
                .into_transfer_to_initializer_context()
                .with_signer(&[&authority_seeds[..]]),
            STAKING_AMOUNT,
        )?;

        token::close_account(
            ctx.accounts
                .into_close_context()
                .with_signer(&[&authority_seeds[..]]),
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(vault_account_bump: u8)]
pub struct Stake<'info> {
    
    #[account(mut, signer)] 
    /// CHECK: this is safe because we have run client-side validation on the wallet initializing the transaction
    pub staking_token_owner: AccountInfo<'info>,
    pub staking_mint: Account<'info, Mint>, 
    #[account(
        init,
        seeds = [b"receipt".as_ref(), staking_account.key().as_ref(), staking_mint.key().as_ref()],
        bump,
        payer = staking_token_owner,
        token::mint = staking_mint,
        token::authority = staking_token_owner,
        
    )]
    pub vault_account: Account<'info, TokenAccount>, 
    #[account(
        mut,
        constraint = owner_staking_token_account.amount == STAKING_AMOUNT
    )]
    pub owner_staking_token_account: Account<'info, TokenAccount>,
    #[account(init, payer = staking_token_owner, space = 8 + StakeAccount::LEN)]
    pub staking_account: Box<Account<'info, StakeAccount>>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
    pub token_program: Program<'info, Token>,
}

impl<'info> Stake<'info> {
    fn into_transfer_to_pda_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self
            .owner_staking_token_account
            .to_account_info()
            .clone(),
            to: self.vault_account.to_account_info().clone(),
            authority: self.staking_token_owner.clone(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
    
    fn into_set_authority_context(&self) -> CpiContext<'_, '_, '_, 'info, SetAuthority<'info>> {
        let cpi_accounts = SetAuthority {
            account_or_mint: self.vault_account.to_account_info().clone(),
            current_authority: self.staking_token_owner.clone(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
}

#[derive(Accounts)]
pub struct Collect<'info> {
    /// CHECK: this is safe because the mint authority is stored in an environmental variable
    #[account(signer)]
    pub reward_mint_authority: AccountInfo<'info>,
    /// CHECK: this is safe because we have run client-side validation on the wallet initializing the transaction
    pub staking_token_owner: AccountInfo<'info>,
    #[account(mut)]
    pub owner_staking_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = staking_account.staking_token_owner == *staking_token_owner.key,
        constraint = staking_account.owner_staking_token_account == *owner_staking_token_account.to_account_info().key,
        constraint = staking_account.staking_mint == *staking_mint.to_account_info().key,
        has_one = staking_token_owner,
        has_one = owner_staking_token_account,
    )]
    pub staking_account: Box<Account<'info, StakeAccount>>,
    pub staking_mint: Account<'info, Mint>,
    #[account(mut)]
    pub reward_mint: Account<'info, Mint>,
    #[account(mut)]
    pub owner_reward_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

impl<'info> Collect<'info> {
    fn into_mint_to_staker(&self) -> CpiContext<'_, '_, '_, 'info, MintTo<'info>> {
        let cpi_accounts = MintTo {
            mint: self.reward_mint.to_account_info().clone(),
            to: self.owner_reward_token_account.to_account_info().clone(),
            authority: self.reward_mint_authority.clone(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
}

#[derive(Accounts)]
pub struct Unstake<'info> {
    /// CHECK: this is safe because the mint authority is stored in an environmental variable
    #[account(mut, signer)] 
    pub staking_token_owner: AccountInfo<'info>,
    pub staking_mint: Account<'info, Mint>, 
    #[account(mut)] 
    pub vault_account: Account<'info, TokenAccount>,
    /// CHECK: 
    pub vault_authority: AccountInfo<'info>,
    #[account(mut)]
    pub owner_staking_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = staking_account.staking_token_owner == *staking_token_owner.key,
        constraint = staking_account.owner_staking_token_account == *owner_staking_token_account.to_account_info().key,
        constraint = staking_account.staking_mint == *staking_mint.to_account_info().key,
        has_one = staking_token_owner,
        has_one = owner_staking_token_account,
        has_one = staking_mint,
        close = staking_token_owner
    )] 
    pub staking_account: Box<Account<'info, StakeAccount>>,
    pub token_program: Program<'info, Token>,
}

impl<'info> Unstake<'info> {
    fn into_transfer_to_initializer_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.vault_account.to_account_info().clone(),
            to: self
            .owner_staking_token_account
            .to_account_info()
            .clone(),
            authority: self.vault_authority.clone(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
    
    fn into_close_context(&self) -> CpiContext<'_, '_, '_, 'info, CloseAccount<'info>> {
        let cpi_accounts = CloseAccount {
            account: self.vault_account.to_account_info().clone(),
            destination: self.staking_token_owner.clone(),
            authority: self.vault_authority.clone(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
}

#[account]
pub struct StakeAccount {
    pub staking_token_owner: Pubkey,
    pub owner_staking_token_account: Pubkey,
    pub staking_mint: Pubkey,
    pub created: i64,
    pub last_reward_collection: i64,
    pub total_reward_collected: u64,
    pub earliest_unstake: i64,
}

const DISCRIMINATOR_LENGTH: usize = 8;
const PUBLIC_KEY_LENGTH: usize = 32;
const TIMESTAMP_LENGTH: usize = 8;

impl StakeAccount {
    const LEN: usize = DISCRIMINATOR_LENGTH
        + PUBLIC_KEY_LENGTH // initializer
        + PUBLIC_KEY_LENGTH // initializer_deposit_token_account
        + PUBLIC_KEY_LENGTH // mint
        + TIMESTAMP_LENGTH // created
        + TIMESTAMP_LENGTH // last_reward_collection
        + 8 // total_reward_collected
        + TIMESTAMP_LENGTH; // earliest_unstake
}

#[error]
pub enum ErrorCode {
    #[msg("You have not waited long enough since your last reward collection.")]
    CollectionPeriodTooShort,
    #[msg("You have not waited long enough since you staked this token.")]
    StakingPeriodTooShort
}