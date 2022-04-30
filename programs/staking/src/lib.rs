use anchor_lang::prelude::*;
use anchor_spl::token::{self, CloseAccount, Mint, Token, SetAuthority, MintTo, TokenAccount, Transfer};
use spl_token::instruction::AuthorityType;

declare_id!("5QWdVhYaHiwtXLrbzRwMUmvFJuCL2MHkfza3ro3RuQnE");


const VAULT_PDA_SEED: &[u8] = b"vault";
const STAKING_AMOUNT: u64 = 1;
const MINIMUM_COLLECTION_PERIOD: i64 = 86400; // 1 day in seconds

// Reward tiers for standard collection
const ONE_WEEK_REWARD: i64 = 35;
const TWO_WEEK_REWARD: i64 = 98;
const FOUR_WEEK_REWARD: i64 = 280;

// Reward tiers for One-of-One tokens (OOO)
const ONE_WEEK_REWARD_OOO: i64 = 49;
const TWO_WEEK_REWARD_OOO: i64 = 140;
const FOUR_WEEK_REWARD_OOO: i64 = 420;

// Staking period tiers, duration in seconds. Alternative shorter periods for testing purposes in comments
// const STAKING_PERIOD_ONE_WEEK: i64 = 604800;
const STAKING_PERIOD_ONE_WEEK: i64 = 60;
// const STAKING_PERIOD_TWO_WEEK: i64 = 1209600;
const STAKING_PERIOD_TWO_WEEK: i64 = 20;
// const STAKING_PERIOD_FOUR_WEEK: i64 = 2419200;
const STAKING_PERIOD_FOUR_WEEK: i64 = 30;

// Size constants
const DISCRIMINATOR_LENGTH: usize = 8;
const PUBLIC_KEY_LENGTH: usize = 32;
const TIMESTAMP_LENGTH: usize = 8;

// Mint Information
const MINT_AUTHORITY_PUBLIC_KEY: &str = "3iJqzWcBEmjrvDKuWMAzgKnfqnWbqcaQc9kvYbHJg1gf";
const MINT_ADDRESS: &str = "MAGf4MnUUkkAUUdiYbNFcDnE4EBGHJYLk9foJ2ae7BV";
// This might be a better way, but Anchor is not recognizing the macro
// const MINT_AUTHORITY_PUBLIC_KEY: Pubkey = pubkey!("3iJqzWcBEmjrvDKuWMAzgKnfqnWbqcaQc9kvYbHJg1gf"); 
// const MINT_ADDRESS: Pubkey = pubkey!("MAGf4MnUUkkAUUdiYbNFcDnE4EBGHJYLk9foJ2ae7BV");

#[program]
pub mod staking {
    use super::*;
    // Allow user to stake a single NFT
    pub fn stake(ctx: Context<Stake>, staking_period: u16, is_one_of_one: bool) -> ProgramResult {

        // Check that the staking period is valid
        if staking_period > 2 {
            return Err(ErrorCode::InvalidStakingPeriod.into())
        }

        // Define variables based on args/time
        let clock: Clock = Clock::get().unwrap();
        let timestamp = clock.unix_timestamp;

        let mut unstake: i64 = 0;

        if staking_period == 0 {
            unstake = STAKING_PERIOD_ONE_WEEK;
        } else if staking_period == 1 {
            unstake = STAKING_PERIOD_TWO_WEEK;
        } else if staking_period == 2 {
            unstake = STAKING_PERIOD_FOUR_WEEK;
        }

        // Define properties of staking_account account that will be created as a record of the staked token
        ctx.accounts.staking_account.staking_token_owner = *ctx.accounts.staking_token_owner.key;
        ctx.accounts
            .staking_account
            .owner_staking_token_account = *ctx
            .accounts
            .owner_staking_token_account
            .to_account_info()
            .key;
        ctx.accounts.staking_account.staking_mint = *ctx.accounts.staking_mint.to_account_info().key;

        if is_one_of_one == true {
            ctx.accounts.staking_account.is_one_of_one = true;
        } else {
            ctx.accounts.staking_account.is_one_of_one = false;
        }

        ctx.accounts.staking_account.created = timestamp;

        ctx.accounts.staking_account.owner_reward_token_account = *ctx.accounts.owner_reward_token_account.to_account_info().key;
        ctx.accounts.staking_account.last_reward_collection = timestamp;
        ctx.accounts.staking_account.total_reward_collected = 0;

        ctx.accounts.staking_account.staking_period = staking_period;
        ctx.accounts.staking_account.unstake_date = timestamp + unstake;

        // Set authority for staking vault (PDA)
        let (vault_authority, _vault_authority_bump) =
            Pubkey::find_program_address(&[VAULT_PDA_SEED, &*ctx.accounts.staking_account.to_account_info().key.as_ref(), &*ctx.accounts.staking_mint.to_account_info().key.as_ref()], ctx.program_id);

        token::set_authority(
            ctx.accounts.into_set_authority_context(),
            AuthorityType::AccountOwner,
            Some(vault_authority),
        )?;

        // Transfer token to PDA
        token::transfer(
            ctx.accounts.into_transfer_to_pda_context(),
            STAKING_AMOUNT,
        )?;

        Ok(())
    }

    // Allow for collection of rewards over the course of staking period. 
    // Must allow at least one day to pass in between collection attempts
    pub fn collect(ctx: Context<Collect>) -> ProgramResult {

        // Define time-related variables
        let clock: Clock = Clock::get().unwrap();
        let timestamp = clock.unix_timestamp;
        let elapsed: i64 = timestamp - ctx.accounts.staking_account.last_reward_collection;

        // Check that minimum collection time has elapsed
        if elapsed < MINIMUM_COLLECTION_PERIOD {
            return Err(ErrorCode::NotEnoughElapsedSinceLastCollection.into())
        }

        // Check that the reward has not already been fully collected - SATISFY SOTERIA ISSUE M-2
        if ctx.accounts.staking_account.full_reward_collected == true {
             return Err(ErrorCode::FullRewardAlreadyCollected.into())
        }

        // Check that the staking period is valid
        if ctx.accounts.staking_account.staking_period > 2 {
            return Err(ErrorCode::InvalidStakingPeriod.into())
        }

        // Establish number of full days that have passed since staking/last collection
        let days = elapsed / MINIMUM_COLLECTION_PERIOD;
        let mut amount: i64 = 0;
        let mut full_amount: i64 = 0;

        // Define the "per diem" rate of the staking period and multiply by "days" to determine amount to be rewarded. 
        // Define the full_amount based on the staking period.
        if ctx.accounts.staking_account.is_one_of_one == true {
            if ctx.accounts.staking_account.staking_period == 0 {
                amount = (ONE_WEEK_REWARD_OOO as i64 / 7) * days as i64;
                full_amount = ONE_WEEK_REWARD_OOO
            } else if ctx.accounts.staking_account.staking_period == 1 {
                amount = (TWO_WEEK_REWARD_OOO as i64 / 14) * days as i64;
                full_amount = TWO_WEEK_REWARD_OOO
            } else if ctx.accounts.staking_account.staking_period == 2 {
                amount = (FOUR_WEEK_REWARD_OOO as i64 / 28) * days as i64;
                full_amount = FOUR_WEEK_REWARD_OOO
            }
        } else {
            if ctx.accounts.staking_account.staking_period == 0 {
                amount = (ONE_WEEK_REWARD as i64 / 7) * days as i64;
                full_amount = ONE_WEEK_REWARD
            } else if ctx.accounts.staking_account.staking_period == 1 {
                amount = (TWO_WEEK_REWARD as i64 / 14) * days as i64;
                full_amount = TWO_WEEK_REWARD
            } else if ctx.accounts.staking_account.staking_period == 2 {
                amount = (FOUR_WEEK_REWARD as i64 / 28) * days as i64;
                full_amount = FOUR_WEEK_REWARD
            }
        }

        // Check that the reward has not already been fully collected
        if ctx.accounts.staking_account.total_reward_collected >= full_amount{
             return Err(ErrorCode::FullRewardAlreadyCollected.into())
        }

        // Catch cases that might results in the staking_token_owner collecting more than the full_amount
        if amount > full_amount || amount + ctx.accounts.staking_account.total_reward_collected > full_amount{
            amount = full_amount - ctx.accounts.staking_account.total_reward_collected;
        }

        // Mint that amount of reward tokens due, and transfer to the user
        token::mint_to(ctx.accounts.into_mint_to_staker(), amount as u64).unwrap();

        // Update the total amount reward for the staked token and the time of the last collection
        ctx.accounts.staking_account.last_reward_collection = timestamp;
        ctx.accounts.staking_account.total_reward_collected = ctx.accounts.staking_account.total_reward_collected + amount;

        // Check if the full amount has been collected, and update the account accordingly if so
        if ctx.accounts.staking_account.total_reward_collected + amount >= full_amount {
            ctx.accounts.staking_account.full_reward_collected = true
        }

        Ok(())
    }

    // This function is run as the first step of the unstaking process. 
    // This ensures that, whether or not the user has collected rewards along the way, that all rewards due to them are issued before the token is unstaked. 
    pub fn collect_full(ctx: Context<CollectFull>) -> ProgramResult {

        // Define time-related variables
        let clock: Clock = Clock::get().unwrap();
        let timestamp = clock.unix_timestamp;

        // Ensure that the staking period has passed
        if ctx.accounts.staking_account.unstake_date > timestamp {
             return Err(ErrorCode::TooEarlyToUnstake.into())
        }

        // Check that the reward has not already been fully collected
        if ctx.accounts.staking_account.full_reward_collected == true {
             return Err(ErrorCode::FullRewardAlreadyCollected.into())
        }

        // Check that the staking period is valid
        if ctx.accounts.staking_account.staking_period > 2 {
            return Err(ErrorCode::InvalidStakingPeriod.into())
        }
        
        // Determine the full amount due for the staking period
        let mut amount: i64 = 0;
        let mut full_amount: i64 = 0;

        if ctx.accounts.staking_account.is_one_of_one == true {
            if ctx.accounts.staking_account.staking_period == 0 {
                amount = ONE_WEEK_REWARD_OOO;
                full_amount = ONE_WEEK_REWARD_OOO
            } else if ctx.accounts.staking_account.staking_period == 1 {
                amount = TWO_WEEK_REWARD_OOO;
                full_amount = TWO_WEEK_REWARD_OOO
            } else if ctx.accounts.staking_account.staking_period == 2 {
                amount = FOUR_WEEK_REWARD_OOO;
                full_amount = FOUR_WEEK_REWARD_OOO
            }
        } else {
            if ctx.accounts.staking_account.staking_period == 0 {
                amount = ONE_WEEK_REWARD;
                full_amount = ONE_WEEK_REWARD
            } else if ctx.accounts.staking_account.staking_period == 1 {
                amount = TWO_WEEK_REWARD;
                full_amount = TWO_WEEK_REWARD
            } else if ctx.accounts.staking_account.staking_period == 2 {
                amount = FOUR_WEEK_REWARD;
                full_amount = FOUR_WEEK_REWARD
            }
        }

        if ctx.accounts.staking_account.total_reward_collected >= full_amount  {
            return Err(ErrorCode::FullRewardAlreadyCollected.into())
        }

        // Catch cases that might results in the staking_token_owner collecting more than the full_amount
        if amount > full_amount || amount + ctx.accounts.staking_account.total_reward_collected > full_amount{
            amount = full_amount - ctx.accounts.staking_account.total_reward_collected;
        }

        // Subtract any rewards collected along the way from the total reward amount for the staking period
        amount = amount - ctx.accounts.staking_account.total_reward_collected; 

        // Mint the balanace due nd transfer to the user
        token::mint_to(ctx.accounts.into_mint_to_staker(), amount as u64).unwrap();

        // Update the staking_account to show that the full reward amount has been issued
        ctx.accounts.staking_account.total_reward_collected = ctx.accounts.staking_account.total_reward_collected + amount;
        ctx.accounts.staking_account.full_reward_collected = true;
        ctx.accounts.staking_account.last_reward_collection = timestamp;

        Ok(())
    }

    // This function is automatically called after successful return of collect_full, and is responsible for unstaking the token, transferring it back to the user, and closing the related staking_account
    pub fn unstake(ctx: Context<Unstake>) -> ProgramResult {

        // Define time-related variables
        let clock: Clock = Clock::get().unwrap();
        let timestamp = clock.unix_timestamp;
        
        // Ensure that the staking period has passed, and that the full reward has been issued
        if ctx.accounts.staking_account.unstake_date > timestamp {
             return Err(ErrorCode::TooEarlyToUnstake.into())
        }

        // Check that the full_amount has been
        if ctx.accounts.staking_account.full_reward_collected == false {
             return Err(ErrorCode::FullRewardNotCollected.into())
        }

        // Define the vault authority and bump, and set the authority seeds to access the vault (PDA)
        let (_vault_authority, vault_authority_bump) =
            Pubkey::find_program_address(&[VAULT_PDA_SEED, &*ctx.accounts.staking_account.to_account_info().key.as_ref(), &*ctx.accounts.staking_mint.to_account_info().key.as_ref()], ctx.program_id);

        let authority_seeds = &[&VAULT_PDA_SEED, &*ctx.accounts.staking_account.to_account_info().key.as_ref(), &*ctx.accounts.staking_mint.to_account_info().key.as_ref(), &[vault_authority_bump]];

        // Transfer the token back to the user and close the staking_account
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
#[instruction(staking_period: u16, is_one_of_one: bool)]
pub struct Stake<'info> {
    #[account(mut)] 
    pub staking_token_owner: Signer<'info>,
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
    pub owner_reward_token_account: Account<'info, TokenAccount>,
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
            authority: self.staking_token_owner.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
    
    fn into_set_authority_context(&self) -> CpiContext<'_, '_, '_, 'info, SetAuthority<'info>> {
        let cpi_accounts = SetAuthority {
            account_or_mint: self.vault_account.to_account_info().clone(),
            current_authority: self.staking_token_owner.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
}

#[derive(Accounts)]
pub struct Collect<'info> {
    /// CHECK: this is safe because the incoming mint authority is being compared with the stored constant
    #[account(
        constraint = *&reward_mint_authority.to_account_info().key.to_string() == MINT_AUTHORITY_PUBLIC_KEY,
    )]
    pub reward_mint_authority: AccountInfo<'info>,
    #[account()]
    pub staking_token_owner: Signer<'info>,
    #[account(
        constraint = staking_account.owner_staking_token_account == *owner_staking_token_account.to_account_info().key,
    )]
    pub owner_staking_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = staking_account.staking_token_owner == *staking_token_owner.key,
        constraint = staking_account.owner_staking_token_account == *owner_staking_token_account.to_account_info().key,
        constraint = staking_account.staking_mint == *staking_mint.to_account_info().key,
    )]
    pub staking_account: Box<Account<'info, StakeAccount>>,
    // This is required to be passed in as an added layer of validation to be checked against the staking account
    pub staking_mint: Account<'info, Mint>,
    #[account(
        constraint = *&reward_mint.to_account_info().key.to_string() == MINT_ADDRESS,
    )]
    pub reward_mint: Account<'info, Mint>,
    #[account(
        constraint = staking_account.owner_reward_token_account == *owner_reward_token_account.to_account_info().key,
    )]
    pub owner_reward_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

impl<'info> Collect<'info> {
    fn into_mint_to_staker(&self) -> CpiContext<'_, '_, '_, 'info, MintTo<'info>> {
        let cpi_accounts = MintTo {
            mint: self.reward_mint.to_account_info().clone(),
            to: self.owner_reward_token_account.to_account_info().clone(),
            authority: self.reward_mint_authority.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
}

#[derive(Accounts)]
pub struct CollectFull<'info> {
    /// CHECK: this is safe because the incoming mint authority is being compared with the stored constant
    #[account(mut,
        constraint = *&reward_mint_authority.to_account_info().key.to_string() == MINT_AUTHORITY_PUBLIC_KEY,
    )]
    pub reward_mint_authority: AccountInfo<'info>,
    #[account()]
    pub staking_token_owner: Signer<'info>,
    #[account(
        constraint = staking_account.owner_staking_token_account == *owner_staking_token_account.to_account_info().key,
    )]
    pub owner_staking_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = staking_account.staking_token_owner == *staking_token_owner.key,
        constraint = staking_account.owner_staking_token_account == *owner_staking_token_account.to_account_info().key,
        constraint = staking_account.staking_mint == *staking_mint.to_account_info().key,
    )]
    pub staking_account: Box<Account<'info, StakeAccount>>,
    // This is required to be passed in as an added layer of validation to be checked against the staking account
    pub staking_mint: Account<'info, Mint>,
    #[account(
        constraint = *&reward_mint.to_account_info().key.to_string() == MINT_ADDRESS,
    )]
    pub reward_mint: Account<'info, Mint>,
    #[account(
        constraint = staking_account.owner_reward_token_account == *owner_reward_token_account.to_account_info().key,
    )]
    pub owner_reward_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

impl<'info> CollectFull<'info> {
    fn into_mint_to_staker(&self) -> CpiContext<'_, '_, '_, 'info, MintTo<'info>> {
        let cpi_accounts = MintTo {
            mint: self.reward_mint.to_account_info().clone(),
            to: self.owner_reward_token_account.to_account_info().clone(),
            authority: self.reward_mint_authority.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
}

#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut)] 
    pub staking_token_owner: Signer<'info>,
    pub staking_mint: Account<'info, Mint>, 
    #[account(mut)] 
    pub vault_account: Account<'info, TokenAccount>,
    /// CHECK: this is safe because it is calculated by the client
    pub vault_authority: AccountInfo<'info>,
    #[account(mut, 
        seeds = [b"receipt".as_ref(), staking_account.key().as_ref(), staking_mint.key().as_ref()],
        bump,
    )]
    pub owner_staking_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = staking_account.staking_token_owner == *staking_token_owner.key,
        constraint = staking_account.owner_staking_token_account == *owner_staking_token_account.to_account_info().key,
        constraint = staking_account.staking_mint == *staking_mint.to_account_info().key,
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
            destination: self.staking_token_owner.to_account_info().clone(),
            authority: self.vault_authority.clone(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
}

#[account]
pub struct StakeAccount {
    pub staking_token_owner: Pubkey,
    pub owner_staking_token_account: Pubkey,
    pub owner_reward_token_account: Pubkey,
    pub staking_mint: Pubkey,
    pub created: i64,
    pub last_reward_collection: i64,
    pub total_reward_collected: i64,
    pub unstake_date: i64,
    pub staking_period: u16,
    pub is_one_of_one: bool,
    pub full_reward_collected: bool
}

impl StakeAccount {
    const LEN: usize = DISCRIMINATOR_LENGTH
        + PUBLIC_KEY_LENGTH // owner
        + PUBLIC_KEY_LENGTH // owner_staking_token_account
        + PUBLIC_KEY_LENGTH // owner_reward_token_account
        + PUBLIC_KEY_LENGTH // mint
        + TIMESTAMP_LENGTH // created
        + TIMESTAMP_LENGTH // last_reward_collection
        + 8 // total_reward_collected
        + TIMESTAMP_LENGTH // unstake_date
        + 32 // staking period
        + 8 // is one of one
        + 8; // reward collected
}

#[error]
pub enum ErrorCode {
    #[msg("Not enough time has elapsed since your last collection.")]
    NotEnoughElapsedSinceLastCollection,
    #[msg("It is too early to unstaked this token.")]
    TooEarlyToUnstake,
    #[msg("The reward has not been collected. Something must have gone wrong.")]
    FullRewardNotCollected,
    #[msg("The reward has already been collected for this staking period.")]
    FullRewardAlreadyCollected,
    #[msg("The staking period is not valid.")]
    InvalidStakingPeriod
}