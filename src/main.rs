use anyhow::{anyhow, Result};
use solana_client::{
    nonblocking::rpc_client::RpcClient,
    nonblocking::pubsub_client::PubsubClient,
    rpc_config::{RpcAccountInfoConfig, RpcTransactionLogsConfig, RpcTransactionLogsFilter},
};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    signature::{Keypair, Signer},
    pubkey::Pubkey,
    native_token::LAMPORTS_PER_SOL,
};
use std::{sync::Arc, fs, str::FromStr};
use serde_json::from_str;
use futures_util::StreamExt;

const RPC_URL: &str = "https://api.devnet.solana.com";
const WS_URL: &str = "wss://api.devnet.solana.com";
const SLAVE_KEYPAIR_PATH: &str = "/home/toikid/.config/solana/slave_keypair.json";
const MASTER_WALLET_PUBKEY_STR: &str = "3SNiaouRZ8kd3T75JEE3DcgzMxAtK569jjnPmtam9bXk";

// Adresses DEVNET des programmes
const RAYDIUM_AMM_V4_PROGRAM_ID_STR: &str = "HWy1jotHpo6UqeQxx49dpYYdQB8wj9Qk9MdxwjLvDHB8"; // OpenBook AMM devnet
const JUPITER_AGGREGATOR_V6_PROGRAM_ID_STR: &str = "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4";

#[tokio::main]
async fn main() -> Result<()> {
    let rpc_client = Arc::new(RpcClient::new_with_commitment(
        RPC_URL.to_string(),
        CommitmentConfig::confirmed(),
    ));
    println!("✅ Connecté au RPC : {}", RPC_URL);

    let pubsub_client_account = PubsubClient::new(WS_URL).await?;
    println!("✅ Connecté au WebSocket (comptes) : {}", WS_URL);

    let latest_blockhash = rpc_client.get_latest_blockhash().await?;
    println!("ℹ️  Dernier blockhash : {:?}", latest_blockhash);

    let slave_keypair_json_string = fs::read_to_string(SLAVE_KEYPAIR_PATH)
        .map_err(|e| anyhow!("Erreur lecture fichier de clé: {}", e))?;

    let keypair_bytes: Vec<u8> = from_str(&slave_keypair_json_string)
        .map_err(|e| anyhow!("Erreur parsing JSON clé: {}", e))?;

    let slave_keypair = Keypair::from_bytes(&keypair_bytes)
        .map_err(|e| anyhow!("Erreur création Keypair : {}", e))?;

    let slave_pubkey = slave_keypair.pubkey();
    println!("🔑 Clé publique bot (slave): {}", slave_pubkey);

    let slave_balance_lamports = rpc_client.get_balance(&slave_pubkey).await?;
    let slave_balance_sol = slave_balance_lamports as f64 / LAMPORTS_PER_SOL as f64;
    println!("💰 Solde du bot : {} lamports ({:.2} SOL)", slave_balance_lamports, slave_balance_sol);

    let master_pubkey = Pubkey::from_str(MASTER_WALLET_PUBKEY_STR)
        .map_err(|e| anyhow!("Erreur parsing pubkey master: {}", e))?;
    println!("🎯 Clé publique master : {}", master_pubkey);

    let raydium_program_pubkey = Pubkey::from_str(RAYDIUM_AMM_V4_PROGRAM_ID_STR)
        .map_err(|e| anyhow!("Erreur parsing Raydium ID: {}", e))?;
    let jupiter_program_pubkey = Pubkey::from_str(JUPITER_AGGREGATOR_V6_PROGRAM_ID_STR)
        .map_err(|e| anyhow!("Erreur parsing Jupiter ID: {}", e))?;

    // Créer la tâche d'écoute des logs Raydium
    let master_pubkey_for_raydium = master_pubkey.clone();
    let raydium_program_id = raydium_program_pubkey.to_string();
    
    tokio::spawn(async move {
        println!("👂 Démarrage de l'écoute des logs Raydium...");
        
        let pubsub_client_raydium = match PubsubClient::new(WS_URL).await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("❌ Erreur connexion WebSocket Raydium: {}", e);
                return;
            }
        };
        println!("✅ Connecté au WebSocket Raydium : {}", WS_URL);

        let (mut raydium_stream, _raydium_subscription_id) = match pubsub_client_raydium
            .logs_subscribe(
                RpcTransactionLogsFilter::Mentions(vec![raydium_program_id]),
                RpcTransactionLogsConfig {
                    commitment: Some(CommitmentConfig::processed()),
                },
            )
            .await 
        {
            Ok(stream) => stream,
            Err(e) => {
                eprintln!("❌ Erreur abonnement logs Raydium: {}", e);
                return;
            }
        };
        println!("📡 Abonnement aux logs Raydium actif.");

        while let Some(response) = raydium_stream.next().await {
            let signature = response.value.signature;
            let logs = response.value.logs;

            if logs.iter().any(|log| log.contains(&master_pubkey_for_raydium.to_string())) {
                println!(
                    "🔴 Log RAYDIUM détecté pour le Master ! Signature: {} Logs: {:?}",
                    signature, logs
                );
            }
        }
        eprintln!("⛔ Écoute des logs Raydium terminée.");
    });

    // Créer la tâche d'écoute des logs Jupiter
    let master_pubkey_for_jupiter = master_pubkey.clone();
    let jupiter_program_id = jupiter_program_pubkey.to_string();
    
    tokio::spawn(async move {
        println!("👂 Démarrage de l'écoute des logs Jupiter...");
        
        let pubsub_client_jupiter = match PubsubClient::new(WS_URL).await {
            Ok(client) => client,
            Err(e) => {
                eprintln!("❌ Erreur connexion WebSocket Jupiter: {}", e);
                return;
            }
        };
        println!("✅ Connecté au WebSocket Jupiter : {}", WS_URL);

        let (mut jupiter_stream, _jupiter_subscription_id) = match pubsub_client_jupiter
            .logs_subscribe(
                RpcTransactionLogsFilter::Mentions(vec![jupiter_program_id]),
                RpcTransactionLogsConfig {
                    commitment: Some(CommitmentConfig::processed()),
                },
            )
            .await 
        {
            Ok(stream) => stream,
            Err(e) => {
                eprintln!("❌ Erreur abonnement logs Jupiter: {}", e);
                return;
            }
        };
        println!("📡 Abonnement aux logs Jupiter actif.");

        while let Some(response) = jupiter_stream.next().await {
            let signature = response.value.signature;
            let logs = response.value.logs;

            if logs.iter().any(|log| log.contains(&master_pubkey_for_jupiter.to_string())) {
                println!(
                    "🟡 Log JUPITER détecté pour le Master ! Signature: {} Logs: {:?}",
                    signature, logs
                );
            }
        }
        eprintln!("⛔ Écoute des logs Jupiter terminée.");
    });

    println!("👁️  Surveillance des changements de solde Master (SOL)...");

    let (mut account_stream, _subscription_id_account) = pubsub_client_account
        .account_subscribe(
            &master_pubkey,
            Some(RpcAccountInfoConfig {
                commitment: Some(CommitmentConfig::processed()),
                encoding: None,
                data_slice: None,
                min_context_slot: None,
            }),
        )
        .await?;
    println!("📡 Abonnement au compte master actif.");

    while let Some(response) = account_stream.next().await {
        let new_balance_lamports = response.value.lamports;
        println!(
            "📥 Changement de solde détecté ! Nouveau solde: {} lamports ({:.2} SOL)",
            new_balance_lamports,
            new_balance_lamports as f64 / LAMPORTS_PER_SOL as f64
        );
    }

    tokio::signal::ctrl_c().await?;
    println!("🛑 Arrêt du bot.");
    Ok(())
}