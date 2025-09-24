use anchor_lang::prelude::*;
use anchor_spl::{  associated_token::AssociatedToken,  token_interface::{set_authority,Transfer, Mint,MintTo, SetAuthority,TokenAccount, TokenInterface,transfer,mint_to}};
use std::mem::size_of;

declare_id!("AtHB3oVn9q5bnYpLXmrHUfX5rrw25Q1bpg9DhfCw2Hcy");

/// CHECK:
#[program]
pub mod staking_token_contract{

    use super::*;

    pub fn initialize(ctx:Context<Initialize>,reward:u64,start_slot:u64,end_slot:u64)->Result<()>{

        let program_config = &mut ctx.accounts.program_config;
        let owner = &ctx.accounts.owner;
        let token_program = &ctx.accounts.token_program;

        program_config.owner = owner.key();
        program_config.reward_rate_per_token_per_slot = reward;
        program_config.token_mint = ctx.accounts.token_mint.key();
        program_config.start_slot = start_slot;
        program_config.end_slot = end_slot;


        let program_auth = &ctx.accounts.program_auth;
        let token_mint = &ctx.accounts.token_mint;

        program_config.auth_bump = ctx.bumps.program_auth;
        program_config.vault_bump = ctx.bumps.program_vault;

        // making an CPI call 
        let cpi_accounts = SetAuthority{
            account_or_mint: token_mint.to_account_info(),
            current_authority:  owner.to_account_info(),  
        };

        let cpi_program = token_program.to_account_info();

        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

        set_authority(
            cpi_ctx, 
            anchor_spl::token_interface::spl_token_2022::instruction::AuthorityType::MintTokens, 
            Some(program_auth.key())
        )?;

        Ok(())
    }


    pub fn stake(ctx:Context<Stake>,amount:u64)->Result<()>{
      msg!("Stake: User {} stakings {} tokens ",ctx.accounts.user.key(),amount);

      require!(amount>0, StakeError::ZeroAMount);

      let pgm_info = &ctx.accounts.program_info;

      let user_info = &mut ctx.accounts.user_info;

      let user_ata = &ctx.accounts.user_ata;

      let pgm_authority = &ctx.accounts.program_auth;

      let token_vault = &ctx.accounts.token_vault;

      let token_pgm = &ctx.accounts.token_program;

      let clock = Clock::get()?;

      let signer = &ctx.accounts.user;

      if user_info.amount>0 {
        // user has staked earlier, first calculate the reward_debt
        let rewards = calculate_reward(&user_info, &pgm_info, clock.slot)?;

        if rewards>0 {
            mint_reward(rewards, &ctx.accounts.token_mint, user_ata, pgm_authority, &pgm_info, &ctx.accounts.token_program)?;
        }
      }


      // Transfer user token to our valut 
      let cpi_accounts = Transfer{
        authority: signer.to_account_info(),
        from:user_ata.to_account_info(),
        to:token_vault.to_account_info(),
      };

      let transfer_cpi_program = token_pgm.to_account_info();


      let cpi_context = CpiContext::new(transfer_cpi_program, cpi_accounts);

      transfer(
        cpi_context,
        amount
      )?;

    //   update the user_info

    user_info.amount = user_info.amount.checked_add(amount).ok_or(StakeError::Overflow)?;
    user_info.deposit_slot = clock.slot;
    user_info.reward_debt = 0; // reset as user is already rewarded


      Ok(())
    }

    pub fn un_stake(ctx:Context<UnStake>)->Result<()>{
        msg!("Unstake: User {} unstaking all tokens", ctx.accounts.user.key());

        let clock = Clock::get()?;

        let pgm_info = &ctx.accounts.pgm_info;
        let user_info = &mut ctx.accounts.user_info;
        let user_ata = &ctx.accounts.user_ata;
        let token_mint = &ctx.accounts.token_mint;
        // have the authority of vault
        let pgm_auth = &ctx.accounts.mint_auth;
        let token_pgm = &ctx.accounts.token_pgm;
        let vault = &ctx.accounts.vault;

        let rewards = calculate_reward(user_info, &pgm_info, clock.slot)?;

        if rewards>0 {
            mint_reward(rewards, token_mint, user_ata, pgm_auth, pgm_info, token_pgm)?;
        }

        let amount_to_unstake = user_info.amount;

        require!(amount_to_unstake>0,StakeError::NotStaked);

        // CPI To transfer token also need to send the seeds

        let bump = &[pgm_info.auth_bump];

        let seeds = &[
            b"auth".as_ref(), 
            bump
        ][..];

        // Here we are transferring token from our vault to another ata
        let cpi_acccounts = Transfer{
            authority: pgm_auth.to_account_info(),
            from:vault.to_account_info(),
            to: user_ata.to_account_info(),
        };

        let signer_seeds = &[seeds];

        let cpi_program = token_pgm.to_account_info();

        let cpi_context = CpiContext::new_with_signer(cpi_program, cpi_acccounts, signer_seeds);

        transfer(cpi_context, amount_to_unstake)?;

        
        // The user_info account is closed and rent refunded to the user.
        // The state is effectively reset.

        Ok(())
    }

    pub fn claim_points(ctx:Context<ClaimPoints>)->Result<()>{

        msg!("Claim Rewards: User {} claiming rewards", ctx.accounts.user.key());

        let clock = Clock::get()?;
        let pgm_info = &ctx.accounts.pgm_info;
        let user_info= &mut ctx.accounts.user_info;
        let user_ata = &ctx.accounts.user_ata;
        let mint_auth = &ctx.accounts.mint_auth;
        let token_pgm = &ctx.accounts.token_pgm;
        let token_mint = &ctx.accounts.token_mint;

        let mut rewards = calculate_reward(user_info, pgm_info, clock.slot)?;

        msg!("Hardcoded rewards: {}", rewards);
        
        require!(rewards>0,StakeError::ZeroAMount);

        mint_reward(rewards, token_mint, user_ata, mint_auth, pgm_info, token_pgm)?;
        msg!("Rewards minted successfully!");

        user_info.deposit_slot = clock.slot;
        user_info.reward_debt = 0;

        Ok(())
    }

}


fn calculate_reward(
    user_info:&UserInfo,
    contract_info: &ContractInfo,
    current_slot:u64
)->Result<u64>{
    let time_elapsed = current_slot.checked_sub(user_info.deposit_slot).ok_or(StakeError::Underflow)?;

    let result = user_info.amount.checked_mul(time_elapsed).ok_or(StakeError::Underflow)?
        .checked_mul(contract_info.reward_rate_per_token_per_slot).ok_or(StakeError::Overflow)?
    .checked_sub(user_info.reward_debt).ok_or(StakeError::Underflow)?;

    Ok(result)
}

fn mint_reward<'info>(
    amount:u64,
    mint: &InterfaceAccount<'info,Mint>,
    to: &InterfaceAccount<'info,TokenAccount>,
    authority:&AccountInfo<'info>,
    program_info:&Account<'info,ContractInfo>,
    token_program: &Interface<'info,TokenInterface>,
)->Result<()>{

    msg!("mint_reward called with amount: {}", amount);
    msg!("Minting to account: {}", to.key());
    msg!("Using authority: {}", authority.key());

//  Cpi Call karni hai - Pda has authority so have to pass the Seeds 
    let bump = &[program_info.auth_bump];
    let signer_seeds = &[&[
        b"auth".as_ref(),
        bump,
    ][..]];

    let cpi_accounts = MintTo{
        authority:authority.to_account_info(),
        mint:mint.to_account_info(),
        to:to.to_account_info()
    };

    let cpi_program = token_program.to_account_info();

    let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);

    msg!("About to call mint_to with amount: {}", amount);
    mint_to(cpi_ctx, amount)?;
    msg!("mint_to completed successfully");


    Ok(())
}

#[account]
pub struct ContractInfo{
    pub owner:Pubkey,
    pub start_slot:u64,
    pub end_slot:u64,
    pub token_mint:Pubkey,
    pub reward_rate_per_token_per_slot:u64,
    pub auth_bump:u8,
    pub vault_bump:u8,
}

#[account]
pub struct UserInfo{
    pub amount:u64,
    pub deposit_slot:u64,
    pub reward_debt:u64
}

// Intializing Contract 
// 1) PDA - to store all account config
// 2) Token min create karna hai
// 3) PDA to have a authority to sign 
// 4) PDA created from token that can store all the token - vault
#[derive(Accounts)]
pub struct Initialize<'info>{
    #[account(mut)]
    pub owner : Signer<'info>,

    #[account(
        init,
        payer = owner,
        space = 8 + size_of::<ContractInfo>(),
        seeds = [b"config"],
        bump
    )]
    pub program_config : Account<'info , ContractInfo>,

    // have to change the token authority to a PDA so that pgm can freely sign it 
    #[account(
        mut,
        mint::authority = owner,
    )]
    pub token_mint : InterfaceAccount<'info,Mint>,

    // Inside the function we have to make a CPI call to pass the authority to this 
      /// CHECK: 
    #[account(
        seeds = [b"auth"],
        bump
    )]
    pub program_auth : AccountInfo<'info>,

    #[account(
        init,
        payer = owner,
        seeds = [
            b"vault", token_mint.key().as_ref()
        ],
        bump,
        token::mint = token_mint,
        token::authority = program_auth,
    )]
    pub program_vault: InterfaceAccount<'info,TokenAccount>,

    pub token_program : Interface<'info,TokenInterface>,
    pub system_program : Program<'info, System>,
}



#[derive(Accounts)]
pub struct Stake<'info>{

    #[account(mut)]
    pub user : Signer<'info>,

    #[account(
        seeds = [b"config"],
        bump
    )]
    pub program_info : Account<'info,ContractInfo>,

    #[account(
        // Risky to use init_if_needed , always pair it with seed and verify user 
        init_if_needed,
        payer = user,
        space = 8+ size_of::<UserInfo>(),
        seeds = [
            b"user-info",
            user.key().as_ref()
        ],
        bump

    )]
    pub user_info : Account<'info,UserInfo>,


    #[account(
        mut,
        address = program_info.token_mint @StakeError::InvalidMint
    )]
    pub token_mint : InterfaceAccount<'info,Mint>,

    #[account(
        mut,
        associated_token::authority = user,
        associated_token::mint = token_mint,

    )]
    pub user_ata : InterfaceAccount<'info,TokenAccount>,

    #[account(
        mut,
        seeds = [b"vault",token_mint.key().as_ref()],
        bump = program_info.vault_bump,
    )]
    pub token_vault : InterfaceAccount<'info,TokenAccount>,

    /// CHECK:
    #[account(
        seeds = [b"auth"],
        bump = program_info.auth_bump
    )]
    pub program_auth : AccountInfo<'info>,

    pub system_program : Program<'info, System>,
    pub token_program : Interface<'info,TokenInterface>,
    pub ata_program : Program<'info,AssociatedToken>,
}


#[derive(Accounts)]
pub struct UnStake<'info>{
    /// CHECK: 
    #[account(mut)]
    pub user:Signer<'info>,

    #[account(
        mut,
        seeds = [
            b"user-info",
            user.key().as_ref()
        ],
        bump,
        // close this account and refund the user
        close = user,
    )]
    pub user_info : Account<'info, UserInfo>,

    #[account(
        seeds = [b"config"],
        bump 
    )]
    pub pgm_info : Account<'info,ContractInfo>,

    #[account(
        mut,
        address = pgm_info.token_mint @StakeError::InvalidMint
    )]
    pub token_mint : InterfaceAccount<'info,Mint>,

    #[account(
        mut,
        associated_token::authority = user,
        associated_token::mint = token_mint
    )]
    pub user_ata: InterfaceAccount<'info,TokenAccount>,

    #[account(
        mut,
        seeds = [b"vault",token_mint.key().as_ref()],
        bump = pgm_info.vault_bump
    )]
    pub vault : InterfaceAccount<'info,TokenAccount>,

    /// CHECK:
    #[account(
        seeds = [b"auth"],
        bump = pgm_info.auth_bump
    )] 
    pub mint_auth : AccountInfo<'info>,

    pub token_pgm : Interface<'info,TokenInterface>,

    pub ata_program: Program<'info,AssociatedToken>,

    pub system_pgm : Program<'info,System>,
}


#[derive(Accounts)]
pub struct ClaimPoints<'info>{

    #[account(mut)]
    pub user : Signer<'info>,

    #[account(
        mut,
        seeds = [
            b"user-info",
            user.key().as_ref()
        ],
        bump,
    )]
    pub user_info: Account<'info,UserInfo>,

    #[account(
        seeds = [b"config"],
        bump
    )]
    pub pgm_info : Account<'info,ContractInfo>,

    #[account(
        mut,
        address = pgm_info.token_mint @StakeError::InvalidMint
    )]
    pub token_mint : InterfaceAccount<'info,Mint>,

    #[account(
        mut,
        associated_token::authority = user,
        associated_token::mint = token_mint
    )]
    pub user_ata : InterfaceAccount<'info,TokenAccount>,

    /// CHECK: 
    #[account(
        seeds = [b"auth"],
        bump = pgm_info.auth_bump
    )]
    pub mint_auth : AccountInfo<'info>,

    pub token_pgm : Interface<'info,TokenInterface>,
}

#[error_code]
pub enum StakeError{
    #[msg("start block should be less than end block")]
    InvalidBlockGap,

    #[msg("Arithmatic overflow")]
    Overflow,

    #[msg("Arithmatic Underflow")]
    Underflow,

    #[msg("Unauthorized Access")]
    Unauthorized,

    #[msg("Can't stake Contract Now Time's up")]
    LimitHit,

    #[msg("Reward rate is less than or greater than 0 ")]
    InvalidRewardRate,

    #[msg("Invalid Mint")]
    InvalidMint,

    #[msg("Stake Token Amount is 0")]
    ZeroAMount,

    #[msg("Staked Amount is 0")]
    NotStaked,
}