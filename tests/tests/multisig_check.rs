//! This module provides intergration tests for creating multisig LRC20 transactions.

use bdk::bitcoincore_rpc::RpcApi;
use bitcoin::{OutPoint, PrivateKey, secp256k1::Secp256k1};
use bitcoin_client::RawTx;
use lrc20_rpc_api::transactions::Lrc20TransactionsRpcClient;
use lrcdk::{types::FeeRateStrategy, wallet::SyncOptions};
use once_cell::sync::Lazy;

mod common;
use common::*;

static ISSUER_PRIVATE_KEY: Lazy<PrivateKey> = Lazy::new(|| {
    "cNMMXcLoM65N5GaULU7ct2vexmQnJ5i5j3Sjc6iNnEF18vY7gzn9"
        .parse()
        .expect("Should be valid key")
});

static ALICE_PRIVATE_KEY: Lazy<PrivateKey> = Lazy::new(|| {
    "cUK2ZdLQWWpKeFcrrD7BBjiUsEns9M3MFBTkmLTXyzs66TQN72eX"
        .parse()
        .expect("Should be valid key")
});

static BOB_PRIVATE_KEY: Lazy<PrivateKey> = Lazy::new(|| {
    "cP6bf5irWeAXgoVj9YtcgKerSfPGQMQ48JKGtH1oyqFByb1w3gAD"
        .parse()
        .expect("Should be valid key")
});

static CAROL_PUBKEY: Lazy<bitcoin::secp256k1::PublicKey> = Lazy::new(|| {
    "0373fde54e72b074ba8f56b30acb3d90bbac25e4f1bc62f6918d96badbca1a69b1"
        .parse()
        .expect("Should be valid key")
});

#[tokio::test]
async fn test_create_musig_transaction() -> eyre::Result<()> {
    color_eyre::install()?;
    let blockchain_rpc = setup_rpc_blockchain(&ISSUER_PRIVATE_KEY)?;

    let provider_cfg = bitcoin_provider_config(false);
    let blockchain = setup_blockchain(&provider_cfg);

    let lrc20_client = setup_lrc20_client(LRC20_NODE_URL)?;

    let issuer = setup_wallet_from_provider(*ISSUER_PRIVATE_KEY, provider_cfg.clone()).await?;

    let alice = setup_wallet_from_provider(*ALICE_PRIVATE_KEY, provider_cfg.clone()).await?;

    let secp = Secp256k1::new();

    issuer.sync(SyncOptions::bitcoin_only()).await?;
    if issuer.bitcoin_balances()?.get_spendable() < 100_000 {
        blockchain_rpc.generate_to_address(101, &issuer.address()?)?;
    }
    issuer.sync(SyncOptions::default()).await?;

    let bobs_pubkey = BOB_PRIVATE_KEY.public_key(&secp);
    let alice_pubkey = ALICE_PRIVATE_KEY.public_key(&secp);

    const ISSUANCE_AMOUNT: u128 = 1000;

    let fee_rate_strategy = FeeRateStrategy::Manual { fee_rate: 2.0 };

    // Create issuance with one multisig output to BOB and ALICE
    let issuance = {
        let mut builder = issuer.build_issuance(None)?;

        builder
            // Multisig with 2-of-2
            .add_multisig_recipient(
                vec![bobs_pubkey.inner, alice_pubkey.inner],
                2,
                ISSUANCE_AMOUNT,
                1000,
            )
            // Fund Alice with bitcoins too
            .add_sats_recipient(&alice_pubkey.inner, 10000)
            .set_fee_rate_strategy(fee_rate_strategy);

        builder.finish(&blockchain).await?
    };

    // let fee_rate = fee_rate_strategy.get_fee_rate(&provider)?;

    // TODO: Failed estimation on regtest
    // assert_fee_matches_difference(&issuance, &provider, fee_rate, true)?;

    dbg!(&issuance.tx_type);

    let txid = issuance.bitcoin_tx.txid();

    lrc20_client.send_lrc20_tx(issuance.hex(), None).await?;

    // Add block with issuance to the chain
    blockchain_rpc.generate_to_address(6, &issuer.address()?)?;

    let tx = wait_until_reject_or_attach(txid, &lrc20_client).await?;

    assert_attached!(tx, "Issuance was not accepted by LRC20 node");

    alice.sync(SyncOptions::default()).await?;

    const TRANSFER_AMOUNT: u128 = 100;

    // Create transfer that spends issuance and sends 100 tokens to Carol
    let transfer = {
        let token_pubkey = ISSUER_PRIVATE_KEY.public_key(&secp).into();

        let mut builder = alice.build_transfer()?;

        // NOTE: multisig output should be always first one:
        builder
            .add_2x2multisig_input(OutPoint::new(txid, 1), *BOB_PRIVATE_KEY)
            .add_recipient(token_pubkey, &CAROL_PUBKEY, TRANSFER_AMOUNT, 1000)
            .set_fee_rate_strategy(fee_rate_strategy);

        builder.finish(&blockchain).await?
    };

    // TODO: Failed estimation on regtest
    // assert_fee_matches_difference(&transfer, &provider, fee_rate, false)?;

    println!("{}", transfer.bitcoin_tx.raw_hex());
    dbg!(&transfer.tx_type);

    let txid = transfer.bitcoin_tx.txid();

    lrc20_client.send_lrc20_tx(transfer.hex(), None).await?;

    // Add block with transfer to the chain and sign it
    blockchain_rpc.generate_to_address(6, &alice.address()?)?;

    // Check that the transfer was accepted by LRC20 node
    let tx = wait_until_reject_or_attach(txid, &lrc20_client).await?;

    assert_attached!(tx, "Transfer was not accepted by LRC20 node");

    Ok(())
}
