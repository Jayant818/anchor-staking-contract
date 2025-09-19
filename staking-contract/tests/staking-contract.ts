import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { StakingContract } from "../target/types/staking_contract";

describe("staking-contract", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.stakingContract as Program<StakingContract>;

  it("Is initialized!", async () => {
    // Add your test here.
    const tx = await program.methods.initialize().rpc();
    console.log("Your transaction signature", tx);
  });
});
