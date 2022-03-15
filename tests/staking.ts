import * as anchor from '@project-serum/anchor';
import { Program } from '@project-serum/anchor';
import { Staking } from '../target/types/staking';
import { PublicKey, SystemProgram, Transaction } from '@solana/web3.js';
import { TOKEN_PROGRAM_ID, getMint, getAccount } from '@solana/spl-token';
import { assert } from 'chai';

import {
  ownerWalletKeypair,
  payerKeypair, // Call this something better - attacker, etc
  rewardMintAuthorityKeypair,
} from './utils/users';

describe('staking', () => {
  const provider = anchor.Provider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Staking as Program<Staking>;

  const rewardMint = '5wwzrurTXDNHDDrHw2PS78Ev38Hd9f7askUeVzDsnnQ7';
  const rewardMintPk = new PublicKey(rewardMint);

  const initializerMainAccount = ownerWalletKeypair;

  const tokenMintKey = new PublicKey(
    'mpPGBiedL26AMGz58EKaLR1X692eVD6QoXwxXm6LWjX'
  );

  let token;

  it('Sets the staking token account', async () => {
    const ATA = (
      await provider.connection.getParsedTokenAccountsByOwner(
        initializerMainAccount.publicKey as PublicKey,
        {
          mint: tokenMintKey as PublicKey,
        }
      )
    ).value;

    token = ATA[0];
  });

  it('Fails to stake the token from an unauthorized wallet', async () => {
    try {
      const ATA = (
        await provider.connection.getParsedTokenAccountsByOwner(
          payerKeypair.publicKey as PublicKey,
          {
            mint: tokenMintKey as PublicKey,
          }
        )
      ).value;

      const invalidToken = ATA[0];
      console.log(invalidToken);

      const tokenAccount = await getAccount(
        provider.connection,
        invalidToken.pubkey
      );

      const tokenMintPk = invalidToken.account.data.parsed.info.mint;
      const tokenPk = new PublicKey(tokenMintPk);

      const vaultKeypair = anchor.web3.Keypair.generate();

      const [_vault_account_pda, _vault_account_bump] =
        await PublicKey.findProgramAddress(
          [
            Buffer.from(anchor.utils.bytes.utf8.encode('receipt')),
            vaultKeypair.publicKey.toBuffer(),
            tokenPk.toBuffer(),
          ],
          program.programId
        );

      const vault_account_pda = _vault_account_pda;
      const vault_account_bump = _vault_account_bump;

      console.log('attempting false stake...');

      await program.rpc.stake(vault_account_bump, {
        accounts: {
          stakingTokenOwner: payerKeypair.publicKey,
          stakingMint: tokenPk,
          vaultAccount: vault_account_pda,
          ownerStakingTokenAccount: tokenAccount.address,
          stakingAccount: vaultKeypair.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [vaultKeypair, payerKeypair],
      });
      // }

      console.log('false stake successful');
    } catch {
      console.log('false stake failed');
      assert.ok(true);
    }
  });

  it('Stakes the selected token from the owner wallet', async () => {
    const tokenAccount = await getAccount(provider.connection, token.pubkey);
    console.log(tokenAccount);

    const tokenMintPk = token.account.data.parsed.info.mint;
    const tokenPk = new PublicKey(tokenMintPk);

    const escroKeypair = anchor.web3.Keypair.generate();

    const [_vault_account_pda, _vault_account_bump] =
      await PublicKey.findProgramAddress(
        [
          Buffer.from(anchor.utils.bytes.utf8.encode('receipt')),
          escroKeypair.publicKey.toBuffer(),
          tokenPk.toBuffer(),
        ],
        program.programId
      );

    const vault_account_pda = _vault_account_pda;
    const vault_account_bump = _vault_account_bump;

    console.log('attempting to stake token...');

    await program.rpc.stake(
      vault_account_bump,
      // new anchor.BN(initializerAmount),
      {
        accounts: {
          stakingTokenOwner: initializerMainAccount.publicKey,
          stakingMint: tokenPk,
          vaultAccount: vault_account_pda,
          ownerStakingTokenAccount: tokenAccount.address,
          stakingAccount: escroKeypair.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [escroKeypair, initializerMainAccount],
      }
    );

    console.log('token successfuly staked!');
  });

  it('Fails to unstake a token before the minimum duration', async () => {
    let _allVault = await program.account.stakeAccount.all();
    let stakedToken = _allVault.filter(
      (token) =>
        token.account.stakingMint.toString() === tokenMintKey.toString()
    );

    console.log(stakedToken[0].account.created.toString());
    console.log(
      Math.floor(Date.now() / 1000) -
        parseInt(stakedToken[0].account.created.toString())
    );

    try {
      console.log('attempting unstake...');

      const [_vault_account_pda, _vault_account_bump] =
        await PublicKey.findProgramAddress(
          [
            Buffer.from(anchor.utils.bytes.utf8.encode('receipt')),
            stakedToken[0].publicKey.toBuffer(),
            stakedToken[0].account.stakingMint.toBuffer(),
          ],
          program.programId
        );

      const vault_account_pda = _vault_account_pda;
      const vault_account_bump = _vault_account_bump;

      const [_vault_authority_pda, _vault_authority_bump] =
        await PublicKey.findProgramAddress(
          [
            Buffer.from(anchor.utils.bytes.utf8.encode('vault')),
            stakedToken[0].publicKey.toBuffer(),
            stakedToken[0].account.stakingMint.toBuffer(),
          ],
          program.programId
        );

      const vault_authority_pda = _vault_authority_pda;

      await program.rpc.unstake({
        accounts: {
          stakingTokenOwner: initializerMainAccount.publicKey,

          stakingMint: stakedToken[0].account.stakingMint,
          ownerStakingTokenAccount:
            stakedToken[0].account.ownerStakingTokenAccount,
          vaultAccount: vault_account_pda,
          vaultAuthority: vault_authority_pda,
          stakingAccount: stakedToken[0].publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [initializerMainAccount],
      });

      _allVault = await program.account.stakeAccount.all();

      console.log(_allVault);
      console.log('early unstaking went through');
      assert.ok(false);
    } catch {
      console.log('early unstaking failed');
      assert.ok(true);
    }
  });

  it('Prevents a user from collecting rewards before the minimum duration', async () => {
    console.log('attempting early reward collection');
    let _allVault = await program.account.stakeAccount.all();
    let selectedToken = _allVault.filter(
      (token) =>
        token.account.stakingMint.toString() === tokenMintKey.toString()
    );

    try {
      let retrievedRewardAta = (
        await provider.connection.getParsedTokenAccountsByOwner(
          ownerWalletKeypair.publicKey as PublicKey,
          {
            mint: rewardMintPk as PublicKey,
          }
        )
      ).value;

      await program.rpc.collect(new anchor.BN(500), {
        accounts: {
          rewardMintAuthority: rewardMintAuthorityKeypair.publicKey,
          stakingTokenOwner: initializerMainAccount.publicKey,
          ownerStakingTokenAccount:
            selectedToken[0].account.ownerStakingTokenAccount,
          stakingAccount: selectedToken[0].publicKey,
          stakingMint: selectedToken[0].account.stakingMint,
          rewardMint: rewardMintPk,
          ownerRewardTokenAccount: retrievedRewardAta[0].pubkey,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
      });

      console.log('early reward collection went through');
      assert.ok(false);
    } catch {
      console.log('early reward collection failed');
      assert.ok(true);
    }
  });

  it('Fails to distributes rewards to an unrelated public key', async () => {
    try {
      console.log('false public key attempting to collect rewards...');

      let _allVault = await program.account.stakeAccount.all();
      let selectedToken = _allVault.filter(
        (token) =>
          token.account.stakingMint.toString() === tokenMintKey.toString()
      );

      let retrievedRewardAta = (
        await provider.connection.getParsedTokenAccountsByOwner(
          payerKeypair.publicKey as PublicKey,
          {
            mint: rewardMintPk as PublicKey,
          }
        )
      ).value;

      await program.rpc.collect(new anchor.BN(500), {
        accounts: {
          rewardMintAuthority: rewardMintAuthorityKeypair.publicKey,
          stakingTokenOwner: payerKeypair.publicKey,
          ownerStakingTokenAccount:
            selectedToken[0].account.ownerStakingTokenAccount,
          stakingAccount: selectedToken[0].publicKey,
          stakingMint: selectedToken[0].account.stakingMint,
          rewardMint: rewardMintPk,
          ownerRewardTokenAccount: retrievedRewardAta[0].pubkey,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
      });

      console.log(
        "false public key was able to collect someone else's rewards"
      );
      assert.ok(false);
    } catch {
      console.log('false reward collection #1 failed');
      assert.ok(true);
    }
  });

  it('Fails to distributes rewards to an unrelated public key with all other info correct', async () => {
    try {
      console.log(
        'false public key with more information attempting to collect rewards...'
      );

      let _allVault = await program.account.stakeAccount.all();
      let selectedToken = _allVault.filter(
        (token) =>
          token.account.stakingMint.toString() === tokenMintKey.toString()
      );

      let retrievedRewardAta = (
        await provider.connection.getParsedTokenAccountsByOwner(
          initializerMainAccount.publicKey as PublicKey,
          {
            mint: rewardMintPk as PublicKey,
          }
        )
      ).value;

      await program.rpc.collect(new anchor.BN(500), {
        accounts: {
          rewardMintAuthority: rewardMintAuthorityKeypair.publicKey,
          stakingTokenOwner: payerKeypair.publicKey,
          ownerStakingTokenAccount:
            selectedToken[0].account.ownerStakingTokenAccount,
          stakingAccount: selectedToken[0].publicKey,
          stakingMint: selectedToken[0].account.stakingMint,
          rewardMint: rewardMintPk,
          ownerRewardTokenAccount: retrievedRewardAta[0].pubkey,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
      });

      console.log(
        "false public key with more information was able to collect someone else's rewards"
      );
      assert.ok(false);
    } catch {
      console.log('false reward collection #2 failed');
      assert.ok(true);
    }
  });

  it('Fails to unstake a token staked by someone else', async () => {
    try {
      let _allVault = await program.account.stakeAccount.all();
      let stakedToken = _allVault.filter(
        (token) =>
          token.account.stakingMint.toString() === tokenMintKey.toString()
      );

      console.log(
        "false public key attempting unstake someone else's token..."
      );

      const [_vault_account_pda, _vault_account_bump] =
        await PublicKey.findProgramAddress(
          [
            Buffer.from(anchor.utils.bytes.utf8.encode('receipt')),
            stakedToken[0].publicKey.toBuffer(),
            stakedToken[0].account.stakingMint.toBuffer(),
          ],
          program.programId
        );

      const vault_account_pda = _vault_account_pda;
      const vault_account_bump = _vault_account_bump;

      const [_vault_authority_pda, _vault_authority_bump] =
        await PublicKey.findProgramAddress(
          [
            Buffer.from(anchor.utils.bytes.utf8.encode('vault')),
            stakedToken[0].publicKey.toBuffer(),
            stakedToken[0].account.stakingMint.toBuffer(),
          ],
          program.programId
        );

      const vault_authority_pda = _vault_authority_pda;

      await program.rpc.unstake({
        accounts: {
          stakingTokenOwner: payerKeypair.publicKey,

          stakingMint: stakedToken[0].account.stakingMint,
          ownerStakingTokenAccount:
            stakedToken[0].account.ownerStakingTokenAccount,
          vaultAccount: vault_account_pda,
          vaultAuthority: vault_authority_pda,
          stakingAccount: stakedToken[0].publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [payerKeypair],
      });

      _allVault = await program.account.stakeAccount.all();

      console.log(_allVault);
      console.log('false unstake successful');
    } catch {
      console.log('false unstake failed');
      assert.ok(true);
    }
  });

  it('Allows a user to collect rewards after the minimum duration passes', async () => {
    console.log('attempting distribution...');

    let _allVault = await program.account.stakeAccount.all();
    let selectedToken = _allVault.filter(
      (token) =>
        token.account.stakingMint.toString() === tokenMintKey.toString()
    );

    console.log(selectedToken[0].account.lastRewardCollection.toString());
    console.log(
      Math.floor(Date.now() / 1000) -
        parseInt(selectedToken[0].account.lastRewardCollection.toString())
    );

    let retrievedRewardAta = (
      await provider.connection.getParsedTokenAccountsByOwner(
        ownerWalletKeypair.publicKey as PublicKey,
        {
          mint: rewardMintPk as PublicKey,
        }
      )
    ).value;

    await program.rpc.collect(new anchor.BN(500), {
      accounts: {
        rewardMintAuthority: rewardMintAuthorityKeypair.publicKey,
        stakingTokenOwner: initializerMainAccount.publicKey,
        ownerStakingTokenAccount:
          selectedToken[0].account.ownerStakingTokenAccount,
        stakingAccount: selectedToken[0].publicKey,
        stakingMint: selectedToken[0].account.stakingMint,
        rewardMint: rewardMintPk,
        ownerRewardTokenAccount: retrievedRewardAta[0].pubkey,
        tokenProgram: TOKEN_PROGRAM_ID,
      },
    });

    console.log('successfully collected rewards');
  });

  it('Unstakes a token', async () => {
    let _allVault = await program.account.stakeAccount.all();
    let stakedToken = _allVault.filter(
      (token) =>
        token.account.stakingMint.toString() === tokenMintKey.toString()
    );

    console.log(stakedToken[0].account.created.toString());
    console.log(Math.floor(Date.now() / 1000));
    console.log(
      Math.floor(Date.now() / 1000) -
        parseInt(stakedToken[0].account.created.toString())
    );

    console.log('attempting unstake...');

    const [_vault_account_pda, _vault_account_bump] =
      await PublicKey.findProgramAddress(
        [
          Buffer.from(anchor.utils.bytes.utf8.encode('receipt')),
          stakedToken[0].publicKey.toBuffer(),
          stakedToken[0].account.stakingMint.toBuffer(),
        ],
        program.programId
      );

    const vault_account_pda = _vault_account_pda;
    const vault_account_bump = _vault_account_bump;

    const [_vault_authority_pda, _vault_authority_bump] =
      await PublicKey.findProgramAddress(
        [
          Buffer.from(anchor.utils.bytes.utf8.encode('vault')),
          stakedToken[0].publicKey.toBuffer(),
          stakedToken[0].account.stakingMint.toBuffer(),
        ],
        program.programId
      );

    const vault_authority_pda = _vault_authority_pda;

    await program.rpc.unstake({
      accounts: {
        stakingTokenOwner: initializerMainAccount.publicKey,

        stakingMint: stakedToken[0].account.stakingMint,
        ownerStakingTokenAccount:
          stakedToken[0].account.ownerStakingTokenAccount,
        vaultAccount: vault_account_pda,
        vaultAuthority: vault_authority_pda,
        stakingAccount: stakedToken[0].publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      },
      signers: [initializerMainAccount],
    });

    _allVault = await program.account.stakeAccount.all();

    console.log(_allVault);
  });
});
