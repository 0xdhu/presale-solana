// Migrations are an early feature. Currently, they're nothing more than this
// single deploy script that's invoked from the CLI, injecting a provider
// configured from the workspace's Anchor.toml.

const anchor = require("@project-serum/anchor");
const { ASSOCIATED_TOKEN_PROGRAM_ID, TOKEN_PROGRAM_ID, Token } = require('@solana/spl-token')
const { PublicKey, SystemProgram, SYSVAR_RENT_PUBKEY, Keypair } = anchor.web3;

const idl = require("../target/idl/presale.json");
const programID = idl.metadata.address;

const presale_title = "presale_05";
const pool_usdc = "pool_usdc";
const pool_wen = "pool_wen";

// SEEDED MINT
const wenMint = new PublicKey("Gi3Wnc5TVAus8Zz2rDd193Ujg4snULahKDzvocCMW9cx");
const usdcMint = new PublicKey("4TihkWq855F3GVxA4LGeMQmukRLYfd58c6izMUCp2b5p");

module.exports = async function (provider) {
  // Configure client to use the provider.
  anchor.setProvider(provider);

  // Add your deploy script here.
  const program = new anchor.Program(idl, programID);

  try {
    /* interact with the program via rpc */
    let bumps = {
      presaleAccount: 0,
      poolUsdc: 0,
      poolWen: 0
    };

    // Find PDA from `presale` for state account
    const [presaleAccount, presaleAccountBump] = await PublicKey.findProgramAddress(
      [Buffer.from(presale_title)],
      program.programId
    );
    bumps.presaleAccount = presaleAccountBump;

    // Find PDA from `usdc` for presale pool
    const [poolUsdc, poolUsdcBump] = await PublicKey.findProgramAddress(
      [Buffer.from(presale_title), Buffer.from(pool_usdc)],
      program.programId
    );
    bumps.poolUsdc = poolUsdcBump;

    // Find PDA from `wen` for presale pool
    const [poolWen, poolWenBump] = await PublicKey.findProgramAddress(
      [Buffer.from(presale_title), Buffer.from(pool_wen)],
      program.programId
    );
    bumps.poolWen = poolWenBump;

    console.log("PresaleAccount", presaleAccount.toBase58());
    console.log("Pool-USDC", poolUsdc.toBase58());
    console.log("Pool-WEN", poolWen.toBase58());
    console.log("Bumps", bumps);

    // Signer
    const presaleOwner = provider.wallet.publicKey;
    console.log("PresaleOwner: ", presaleOwner.toBase58(), programID, program.programId.toBase58());

    // initialize
    await program.rpc.initialize(presale_title, bumps, {
      accounts: {
        presaleAccount,
        presaleOwner,
        usdcMint,
        wenMint,
        // poolUsdc,
        // poolWen,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: SYSVAR_RENT_PUBKEY,
      },
    });

  } catch (err) {
    console.log("Transaction error: ", err);
  }
}
