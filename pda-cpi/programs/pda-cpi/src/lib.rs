use anchor_lang::{prelude::*, solana_program::system_instruction::SystemError};

declare_id!("HXShNFBMdtc268HDoevdAwxwqHDVJdyiAmBZ2yTUFNVB");

#[program]
pub mod Counter{

    use anchor_lang::system_program::{transfer, Transfer};

    use super::*;

    pub fn initialize(context:Context<CreatePdaAccount>)->Result<()>{
        Ok(())
    }


    pub fn transfer_without_pda_signing(context:Context<TransferAmountWithoutPDASigining>,amount:u64)->Result<()>{

        let pgm_id = context.accounts.system_program.to_account_info();
        let from_pubKey = context.accounts.Signer.to_account_info();
        let to_pubKey = context.accounts.User_acc.to_account_info();


        let cpi_context = CpiContext::new(pgm_id,Transfer{
            from:from_pubKey,
            to:to_pubKey
        });

        transfer(cpi_context, amount)?;

        Ok(())
    }

    pub fn transfer_with_pda_signing(context:Context<TransferAmountWithoutPDASigining>,amount:u64)->Result<()>{
        
        let pgm_id = context.accounts.system_program.to_account_info();
        let from_pubKey = context.accounts.Signer.to_account_info();
        let to_pubKey = context.accounts.User_acc.to_account_info();
        let bump = context.bumps;
            // Extract bump for PDA
    let bump = *context.bumps.get("pda_account").unwrap();

        let seeds = &[b"client", from_pubKey.key.as_ref(), &[bump]];

        let CpiContext = CpiContext::new(pgm_id,Transfer{
            from:from_pubKey,
            to:to_pubKey
        }).with_signer(&[seeds]);

        transfer(CpiContext, amount)?;
        
        Ok(());
    }
}

// PDA Creation
#[derive(Accounts)]
pub struct CreatePdaAccount<'info>{
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(
        init,
        payer = signer,
        space = 8+32+8+1,
        seeds = [
            b"client",
            signer.key().as_ref()
        ],
        bump
    
    )]
    pub pda_account : Account<'info,PdaAccount>,

    pub system_program: Program<'info,System>,
}


// Transferring without PDA signing 
#[derive(Accounts)]
pub struct TransferAmountWithoutPDASigining<'info>{
    #[account(mut)]
    pub Signer : Signer<'info>,
    #[account(mut)]
    pub User_acc : UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct PdaAccount{
    pub count:u64
}
