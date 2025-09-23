use anchor_lang::{prelude::*, solana_program::stake::state::Stake};

declare_id!("AwZ2jhqFieXREuB6orPsUSnTFssKQCGyhhJ8o6k8G4v1");

const POINTS_PER_DAY:u64 = 1_000_000;
const LAMPORTS_PER_SOL:u64 = 1_000_000_000;
const SECONDS_PER_DAY:u64 = 86_400;

#[program]
pub mod staking_contract{

    use anchor_lang::{solana_program::clock, system_program::{transfer, Transfer}};

    use super::*;


    // Accounts with default data should get created 
    pub fn initialize_pda(context:Context<InitializePda>)->Result<()>{
        let owner = &context.accounts.signer;
        let bump: InitializePdaBumps = context.bumps;

        let  pda_account = &mut context.accounts.pda_account;

        pda_account.bump = bump.pda_account;
        pda_account.owner = *owner.key;
        pda_account.staked_amount = 0;
        pda_account.total_points = 0;

        let clock = clock::Clock::get()?;

        pda_account.last_update_time = clock.unix_timestamp;

        msg!("PDA CREATED successfully");

        Ok(())
    }

    // Staking - Transferring sol from Normal wallet to a PDA 
    // PDA? - seperate PDA for seperate account 
    // we have call transfer method and also sign the pda here 
    // 1) Assuming somehow we are getting the pda address
    //      - then we also got bump from there 
    // 2) Driving the pda from user key  
    //  We are transferring the lamports 
        // 1) either Kisi fixed account mai bhej de and yaha pe bas stakedAmount mai store kar le
        // 2) PDA mai to store nhi kar sakte hum
    pub fn stake(context:Context<StakeIx>,amount:u64)->Result<()>{

        require!(amount>0,StakeError::InvalidAmount);

        let signer = &context.accounts.signer;
        let pda = &mut context.accounts.pda_account;
        let system_program = &context.accounts.system_program;

        let clock = clock::Clock::get()?;

        update_points(pda, clock.unix_timestamp)?;


        let context_input = CpiContext::new(
                system_program.to_account_info()
            , Transfer{
            from: signer.to_account_info(),
            to:pda.to_account_info()
        });

        transfer(context_input, amount)?;

        // also needs to increase the points first, if didn't do that then when we recalcultae then we end up adding more points
        pda.staked_amount = pda.staked_amount.checked_add(amount).ok_or(StakeError::Overflow)?;


        msg!("Staked {} lamports, Total staked: {},Total Points:{}",
            amount, pda.staked_amount, pda.total_points/1_000_000
        );

        Ok(())
    }

    pub fn unstake(context:Context<UnStake>,amount:u64)->Result<()>{

        require!(amount>0,StakeError::InvalidAmount);

        let signer = &context.accounts.signer;
        let pda = &mut context.accounts.pda_account;
        let system_program = & context.accounts.system_program;
        let clock = Clock::get()?;

        require!(pda.staked_amount>=amount, StakeError::InsufficientStake);

        update_points(pda, clock.unix_timestamp);
        let signer_key = context.accounts.signer.key();
        let signer_key_bytes = signer_key.as_ref();

        let seed = &[
            b"client",
            signer_key_bytes,
            &[pda.bump]
        ];

        let seeds: &[&[&[u8]]] = &[seed];


        let cpiContext = CpiContext::new(system_program.to_account_info(), Transfer{
            from: pda.to_account_info(),
            to:signer.to_account_info()
        }).with_signer(seeds);

        transfer(cpiContext,pda.staked_amount)?;

        pda.staked_amount = pda.staked_amount.checked_sub(amount).ok_or(StakeError::Underflow)?;

        msg!("Unstaked {} lamports. Remaining staked: {}, Total points: {}", 
             amount, pda.staked_amount, pda.total_points / 1_000_000);

        Ok(())
    }

    // Claim the points - just return it and make it 0
    pub fn claim_points(context:Context<ClaimPoints>)->Result<()>{
        let pda = &mut context.accounts.pda;
        let clock = Clock::get()?;

        // Update poinst to current time 
        update_points(pda, clock.unix_timestamp);



        let ClaimablePoints = pda.total_points/1_000_000;
        
        msg!("User has {} claimable points",ClaimablePoints);

        pda.total_points = 0;

        Ok(())
    }

    pub fn get_points(context:Context<GetPoints>)->Result<()>{

        let pda_account = &context.accounts.pda;
        let clock = Clock::get()?;

        let time_elapsed = clock.unix_timestamp.checked_sub(pda_account.last_update_time).ok_or(StakeError::InvalidTimestamp)? as u64;
        let new_points = calculate_points_earned(pda_account.staked_amount, time_elapsed)?;

        let current_total_points = pda_account.total_points.checked_add(new_points)
        .ok_or(StakeError::Overflow)?;
    
    msg!("Current points: {}, Staked amount: {} SOL", 
         current_total_points / 1_000_000, 
         pda_account.staked_amount / LAMPORTS_PER_SOL);


        Ok(())
    }
    
}


fn update_points(pda_account: &mut StakeAccount, current_time: i64) -> Result<()> {
    let time_elapsed = current_time.checked_sub(pda_account.last_update_time)
        .ok_or(StakeError::InvalidTimestamp)? as u64;
    
    if time_elapsed > 0 && pda_account.staked_amount > 0 {
        let new_points = calculate_points_earned(pda_account.staked_amount, time_elapsed)?;
        pda_account.total_points = pda_account.total_points.checked_add(new_points)
            .ok_or(StakeError::Overflow)?;
    }
    
    pda_account.last_update_time = current_time;
    Ok(())
}

fn calculate_points_earned(staked_amount:u64,time_elapsed_in_seconds:u64)->Result<(u64)>{
    
    // Points = staked amount * time(in day) * poins per day
    let points = (staked_amount as u128)
        .checked_mul(time_elapsed_in_seconds as u128)
        .ok_or(StakeError::Overflow)?
        .checked_mul(POINTS_PER_DAY as u128)
        .ok_or(StakeError::Overflow)?
        .checked_div(LAMPORTS_PER_SOL as u128)
        .ok_or(StakeError::Overflow)?
        .checked_div(SECONDS_PER_DAY as u128)
        .ok_or(StakeError::Overflow)?;


    Ok(points as u64)
}


#[account]
pub struct StakeAccount{
    pub owner: Pubkey,  // 32 bits
    pub staked_amount : u64, // 64 bits
    pub total_points : u64,
    pub last_update_time:i64,
    pub bump:u8,
}

#[derive(Accounts)]
pub struct InitializePda<'info>{
    #[account(mut)]
    pub signer:Signer<'info>,
    #[account(
        init,
        payer = signer,
        space = 8+4+8+8+8+1,
        seeds = [b"client",signer.key().as_ref()],
        bump
    )]
    pub pda_account:Account<'info,StakeAccount>,
    pub system_program:Program<'info,System>
}


#[derive(Accounts)]
pub struct StakeIx<'info>{
    #[account(mut)]
    pub signer:Signer<'info>,
    #[account(
        mut,
        seeds = [b"client",signer.key().as_ref()],
        bump = pda_account.bump,
        constraint = pda_account.owner == signer.key() @StakeError::Unauthorized
    )]
    pub pda_account : Account<'info,StakeAccount>,
    pub system_program : Program<'info,System>
}

#[derive(Accounts)]
pub struct UnStake<'info>{
    #[account(mut)]
    pub signer:Signer<'info>,
    
    #[account(
        mut,
        seeds = [b"client", signer.key().as_ref()],
        bump = pda_account.bump,
        constraint = pda_account.owner == signer.key() @StakeError::Unauthorized
    )]
    pub pda_account:Account<'info,StakeAccount>,

    pub system_program:Program<'info,System>
}

#[derive(Accounts)]
pub struct ClaimPoints<'info>{
    #[account(mut)]
    pub signer:Signer<'info>,

    #[account(
        mut,
        seeds = [
            b"client",
            signer.key().as_ref()
        ],
        bump = pda.bump
    )]
    pub pda: Account<'info,StakeAccount>
}


// we need to find the Pda somehow using person pub key 
#[derive(Accounts)]
pub struct GetPoints<'info>{
    pub signer: Signer<'info>,
    #[account(
        seeds = [b"client",signer.key().as_ref()],
        bump = pda.bump,
        constraint = pda.owner == signer.key()  @StakeError::Unauthorized
    )]
    pub pda: Account<'info,StakeAccount>
}


#[error_code]
pub enum StakeError {
    #[msg("Amount must be greater than 0")]
    InvalidAmount,
    #[msg("Insufficient staked amount")]
    InsufficientStake,
    #[msg("Unauthorized access")]
    Unauthorized,
    #[msg("Arithmetic overflow")]
    Overflow,
    #[msg("Arithmetic underflow")]
    Underflow,
    #[msg("Invalid timestamp")]
    InvalidTimestamp,
}
