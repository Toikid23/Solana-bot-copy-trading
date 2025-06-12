use anyhow::{anyhow, Result};
use solana_client::{
    nonblocking::rpc_client::RpcClient,
    nonblocking::pubsub_client::PubsubClient,
    rpc_config::RpcAccountInfoConfig,
};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    signature::{Keypair, Signer},
    pubkey::Pubkey,
    native_token::LAMPORTS_PER_SOL,
};
use std::{
    sync::Arc,
    fs,
    str::FromStr,
};
use serde_json::from_str;
use futures_util::StreamExt;

const RPC_URL: &str = "https://api.devnet.solana.com";
const WS_URL: &str = "wss://api.devnet.solana.com";
const SLAVE_KEYPAIR_PATH: &str = "/home/toikid/.config/solana/slave_keypair.json";
const MASTER_WALLET_PUBKEY_STR: &str = "3SNiaouRZ8kd3T75JEE3DcgzMxAtK569jjnPmtam9bXk";

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initialisation du client RPC Solana.
    let rpc_client = Arc::new(RpcClient::new_with_commitment(
        RPC_URL.to_string(),
        CommitmentConfig::confirmed(),
    ));
    println!("Connecté au noeud Solana RPC : {}", RPC_URL);

    // 2. Initialisation du client Pubsub (WebSocket).
    let pubsub_client = PubsubClient::new(WS_URL).await?;
    println!("Connecté au WebSocket Solana PubSub : {}", WS_URL);

    // 3. Récupération du dernier blockhash.
    let latest_blockhash = rpc_client.get_latest_blockhash().await?;
    println!("Dernier blockhash : {:?}", latest_blockhash);

    // 4. Charger la clé privée du portefeuille "slave" (bot).
    let slave_keypair_json_string = fs::read_to_string(SLAVE_KEYPAIR_PATH)
        .map_err(|e| anyhow!("Erreur de lecture du fichier de clé: {}", e))?;

    let keypair_bytes: Vec<u8> = from_str(&slave_keypair_json_string)
        .map_err(|e| anyhow!("Erreur de parsing JSON de la clé: {}", e))?;

    let slave_keypair: Keypair = Keypair::from_bytes(&keypair_bytes)
        .map_err(|e| anyhow!("Erreur de création de Keypair à partir des octets: {}", e))?;

    let slave_pubkey = slave_keypair.pubkey();
    println!("\nClé publique du bot (slave): {}", slave_pubkey);

    // 5. Récupérer le solde du portefeuille "slave".
    let slave_balance_lamports = rpc_client.get_balance(&slave_pubkey).await?;
    let slave_balance_sol = slave_balance_lamports as f64 / LAMPORTS_PER_SOL as f64;
    println!("Solde du bot (slave): {} lamports ({:.2} SOL)", slave_balance_lamports, slave_balance_sol);

    // 6. Convertir la chaîne MASTER_WALLET_PUBKEY_STR en un type Pubkey.
    let master_pubkey = Pubkey::from_str(MASTER_WALLET_PUBKEY_STR)
        .map_err(|e| anyhow!("Erreur de parsing de la clé publique master: {}", e))?;
    println!("\nClé publique du master (à surveiller) : {}", master_pubkey);

    println!("\nSurveillance des changements de solde du Master (SOL)...");

    // 7. Abonnement aux changements de compte du Master via Pubsub.
    let (mut account_stream, _close_future) = pubsub_client
        .account_subscribe(
            &master_pubkey,
            Some(RpcAccountInfoConfig {
                commitment: Some(CommitmentConfig::confirmed()),
                encoding: None,
                data_slice: None,
                min_context_slot: None,
            }),
        )
        .await?;
    println!("Abonnement au compte du Master démarré.");

    // Boucle d'écoute des changements sur le compte du Master.
    while let Some(response) = account_stream.next().await {
        let new_balance_lamports = response.value.lamports;
        println!(
            "DETECTÉ: Changement de solde Master! Nouveau solde: {} lamports ({:.2} SOL)",
            new_balance_lamports,
            new_balance_lamports as f64 / LAMPORTS_PER_SOL as f64
        );
    }

    println!("\nLe programme a terminé avec succès.");

    Ok(())
}