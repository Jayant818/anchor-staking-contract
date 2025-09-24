import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { StakingTokenContract } from "../target/types/staking_token_contract";
import {
  createAssociatedTokenAccount,
  createMint,
  getAssociatedTokenAddress,
  getAssociatedTokenAddressSync,
  getMint,
  mintTo,
  TOKEN_PROGRAM_ID,
  tokenGroupInitializeGroup,
  getAccount,
} from "@solana/spl-token";
import { LAMPORTS_PER_SOL, PublicKey } from "@solana/web3.js";
import { publicKey } from "@coral-xyz/anchor/dist/cjs/utils";
import { assert, expect } from "chai";

describe("staking-token-contract", async () => {
  // Configure the client to use the local cluster.b
  anchor.setProvider(anchor.AnchorProvider.env());

  const provider = anchor.getProvider();

  const program = anchor.workspace
    .stakingTokenContract as Program<StakingTokenContract>;

  // console.log("Program", program);
  const owner = anchor.web3.Keypair.generate();
  const user1 = anchor.web3.Keypair.generate();

  let tokenMint: anchor.web3.PublicKey;
  let user1_ata: anchor.web3.PublicKey;

  // All PDA's
  let config_pda: anchor.web3.PublicKey;
  let auth_pda: anchor.web3.PublicKey;
  let vault_pda: anchor.web3.PublicKey;
  let userInfo_pda: anchor.web3.PublicKey;

  before(async () => {
    const ownerAirdropSignature = await provider.connection.requestAirdrop(
      owner.publicKey,
      anchor.web3.LAMPORTS_PER_SOL
    );
    const userAirdropSignature = await provider.connection.requestAirdrop(
      user1.publicKey,
      anchor.web3.LAMPORTS_PER_SOL
    );

    await provider.connection.confirmTransaction(ownerAirdropSignature);

    await provider.connection.confirmTransaction(userAirdropSignature);

    tokenMint = await createMint(
      provider.connection,
      owner,
      owner.publicKey,
      null,
      9
    );

    user1_ata = await getAssociatedTokenAddress(tokenMint, user1.publicKey);

    await createAssociatedTokenAccount(
      provider.connection,
      user1,
      tokenMint,
      user1.publicKey
    );

    await mintTo(
      provider.connection,
      owner,
      tokenMint,
      user1_ata,
      owner,
      1000 * 10 ** 9
    );

    [config_pda] = await anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("config")],
      program.programId
    );

    [auth_pda] = await anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("auth")],
      program.programId
    );

    [vault_pda] = await anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), tokenMint.toBuffer()],
      program.programId
    );

    [userInfo_pda] = await anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("user-info"), user1.publicKey.toBuffer()],
      program.programId
    );

    console.log("vault address", vault_pda);
    console.log("auth_pda", auth_pda);
    console.log("user_info pda", userInfo_pda);
    console.log("config", config_pda);
    console.log("owner", owner.publicKey);
    console.log("user-1", user1.publicKey);
    console.log("user-1 ata", user1_ata);
  });

  // always wrap numbers in BN
  it("Is initialized!", async () => {
    const rewardPerSlot = new anchor.BN(1);
    const startSlot = new anchor.BN(0);
    const endSlot = new anchor.BN(1000);

    // Create a token Mint from the token Program

    const tx = await program.methods
      .initialize(rewardPerSlot, startSlot, endSlot)
      .accounts({
        owner: owner.publicKey,
        tokenMint: tokenMint,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([owner])
      .rpc();

    // check if the mint authority is transferred or not
    const mintInfo = await getMint(program.provider.connection, tokenMint);

    expect(mintInfo.mintAuthority!.toBuffer()).to.deep.equal(
      auth_pda.toBuffer()
    );
  });

  it("user is Staking tokens", async () => {
    const stakeAmount = new anchor.BN(1 * 10 ** 9);

    const userTokenAccountBefore = await getAccount(
      provider.connection,
      user1_ata
    );

    console.log("userTokenBalance", userTokenAccountBefore.amount);

    const tx = await program.methods
      .stake(stakeAmount)
      .accounts({
        user: user1.publicKey,
        tokenMint: tokenMint,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([user1])
      .rpc();

    const userTokenAccountAfter = await getAccount(
      provider.connection,
      user1_ata
    );

    const expectedBalance =
      userTokenAccountBefore.amount - BigInt(stakeAmount.toString());

    const vault = await getAccount(provider.connection, vault_pda);

    console.log("vault", vault.amount);

    // Also we can check if the vault is increased or not.
    assert.equal(
      userTokenAccountAfter.amount.toString(),
      expectedBalance.toString()
    );
  });

  it("claim_points", async () => {
    const accountBalanceBeforeClaim = (
      await getAccount(provider.connection, user1_ata)
    ).amount;

    console.log("Balance before claiming:", Number(accountBalanceBeforeClaim));

    const stakedAmount = await provider.connection.getAccountInfo(userInfo_pda);

    console.log("stakedAmount", stakedAmount);

    const tx = await program.methods
      .claimPoints()
      .accounts({
        tokenMint: tokenMint,
        tokenPgm: TOKEN_PROGRAM_ID,
        user: user1.publicKey,
      })
      .signers([user1])
      .rpc();

    const accountBalanceAfterClaim = (
      await getAccount(provider.connection, user1_ata)
    ).amount;

    console.log("Balance after claiming:", Number(accountBalanceAfterClaim));
    assert.isAbove(
      Number(accountBalanceAfterClaim),
      Number(accountBalanceBeforeClaim)
    );
  });

  it("Is Unstaking", async () => {
    const vaultBeforeClaim = await getAccount(provider.connection, vault_pda);

    console.log("vaultBeforeClaim", vaultBeforeClaim.amount);

    const accountBalanceBeforeClaim = (
      await getAccount(provider.connection, user1_ata)
    ).amount;

    console.log(
      "Balance before claiming:",
      accountBalanceBeforeClaim.toString()
    );

    const tx = await program.methods
      .unStake()
      .accounts({
        user: user1.publicKey,
        tokenMint: tokenMint,
        tokenPgm: TOKEN_PROGRAM_ID,
      })
      .signers([user1])
      .rpc();

    console.log("Your transaction signature", tx);
    const vault = await getAccount(provider.connection, vault_pda);

    const accountBalanceAfterClaim = (
      await getAccount(provider.connection, user1_ata)
    ).amount;

    console.log("Balance after claiming:", accountBalanceAfterClaim.toString());

    assert.equal(vault.amount, BigInt(0));
  });
});
