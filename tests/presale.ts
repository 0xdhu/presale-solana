import * as anchor from '@project-serum/anchor';
import { Program } from '@project-serum/anchor';
import { Presale } from '../target/types/presale';

describe('presale', () => {

  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.Provider.env());

  const program = anchor.workspace.Presale as Program<Presale>;

  it('Is initialized!', async () => {
    // Add your test here.
    const tx = await program.rpc.initialize({});
    console.log("Your transaction signature", tx);
  });
});
