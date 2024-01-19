mod db;
mod util;
mod wallet;
use bdk::{FeeRate, SignOptions};
use boltz_client::network::electrum::ElectrumConfig;
use boltz_client::network::Chain;
use boltz_client::swaps::bitcoin::{BtcSwapScript, BtcSwapTx};
use boltz_client::util::derivation::SwapKey;
use boltz_client::util::preimage::Preimage;
use boltz_client::KeyPair;
use clap::{Arg, Command};
use db::{create_db, read_db, NetworkInfoModel};
use lightning_invoice::Bolt11Invoice;
use std::path::PathBuf;
use wallet::util::{create_wallet, Descriptors};
const SWAPPY_DIR: &str = ".swappy";
use bdk::bitcoin::{Address, Amount, Transaction};
use bdk::blockchain::{Blockchain, ElectrumBlockchain};
use bdk::{
    database::SqliteDatabase, electrum_client::Client, wallet::AddressIndex::LastUnused,
    SyncOptions, Wallet,
};
use boltz_client::swaps::boltz::{
    BoltzApiClient, CreateSwapRequest, SwapStatusRequest, SwapType, BOLTZ_TESTNET_URL,
};

use std::str::FromStr;
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");
    let api = Command::new("swappy")
        .color(clap::ColorChoice::Always)
        .about("\x1b[0;94mbitc✠in swap wallet\x1b[0m")
        .version("\x1b[0;1mv0.1.3\x1b[0m")
        .subcommand(
            Command::new("create")
                .about("create a wallet with network settings ")
                .display_order(1)
                .args([
                    Arg::new("electrum")
                        .short('e')
                        .long("electrum")
                        .help("electrum server url")
                        .required(true),
                    Arg::new("boltz")
                        .short('b')
                        .long("boltz")
                        .help("boltz server url")
                        .required(true),
                ]),
        )
        .subcommand(
            Command::new("delete")
                .about("delete a wallet (careful!)")
                .display_order(19),
        )
        .subcommand(
            Command::new("read")
                .about("read wallet & network settings ")
                .display_order(2),
        )
        .subcommand(Command::new("sync").about("sync wallet").display_order(3))
        .subcommand(
            Command::new("status")
                .about("wallet balance and history")
                .display_order(4),
        )
        .subcommand(
            Command::new("receive")
                .about("get a bitcoin address or ln invoice to get paid")
                .display_order(5),
        )
        .subcommand(
            Command::new("send")
                .about("pay a bitcoin address or ln invoice")
                .display_order(6),
        )
        .get_matches();

    match api.subcommand() {
        Some(("create", arg_matches)) => {
            let path: PathBuf = match std::env::var("HOME") {
                Ok(home_path) => {
                    let mut full_path = PathBuf::from(home_path);
                    full_path.push(SWAPPY_DIR);
                    full_path
                }
                Err(e) => {
                    eprintln!("Failed to get HOME path: {}", e);
                    return;
                }
            };
            let already_exists = path.exists();
            if already_exists {
                eprintln!("Wallet already exists. Retry after swappy delete.");
                return;
            }

            let mut wallet_info = NetworkInfoModel::from_arg_matches(arg_matches.clone());
            let mnemonic = wallet::util::create_mnemonic();
            let _ = wallet_info.update_mnemonic(mnemonic.clone());
            println!("Your mnemonic is: {}", mnemonic);
            println!("Have you written down and secured your mnemonic? Type 'yes' to confirm:");
            let mut confirmation = String::new();
            std::io::stdin()
                .read_line(&mut confirmation)
                .expect("Failed to read line");
            if confirmation.trim() != "yes" {
                println!("Backup not confirmed. Exiting.");
                return;
            }
            let descriptor = wallet::util::Descriptors::new_public(&mnemonic);

            let response = create_db(wallet_info, &path);
            match response {
                Ok(()) => {
                    println!("Successsfully created new wallet.");
                }
                Err(e) => {
                    eprintln!("{}", e)
                }
            }
        }
        Some(("read", _)) => {
            let path: PathBuf = match std::env::var("HOME") {
                Ok(home_path) => {
                    let mut full_path = PathBuf::from(home_path);
                    full_path.push(SWAPPY_DIR);
                    full_path
                }
                Err(e) => {
                    eprintln!("Failed to get HOME path: {}", e);
                    return;
                }
            };
            let wallet_info = read_db(&path).unwrap();
            println!("{:#?}", wallet_info)
        }

        Some(("delete", _)) => {
            let path: PathBuf = match std::env::var("HOME") {
                Ok(home_path) => {
                    let mut full_path = PathBuf::from(home_path);
                    full_path.push(SWAPPY_DIR);
                    full_path
                }
                Err(e) => {
                    eprintln!("Failed to get HOME path: {}", e);
                    return;
                }
            };
            println!("DELETING WALLET! CAREFUL! ARE YOU SURE? Type 'yes' to confirm.");

            let mut confirmation = String::new();
            std::io::stdin()
                .read_line(&mut confirmation)
                .expect("Failed to read line");
            if confirmation.trim() != "yes" {
                println!("Aborting delete.");
                return;
            }
            if let Err(e) = std::fs::remove_dir_all(path) {
                eprintln!("Failed to delete wallet @ : {}", e);
            } else {
                println!("Wallet successfully deleted.");
            }
        }
        Some(("sync", _)) => {
            let wallet_info = get_wallet_info().unwrap();
            let wallet = init_public_wallet(&wallet_info).unwrap();
            let electrum_url = format!("ssl://{}", wallet_info.electrum_url);
            let client = Client::new(&electrum_url).unwrap();
            let blockchain = ElectrumBlockchain::from(client);
            match wallet.sync(&blockchain, SyncOptions::default()) {
                Ok(()) => {
                    println!("Sync Complete.");
                }
                Err(e) => {
                    eprintln!("Sync Failed: {}", e);
                }
            }
        }
        Some(("status", _)) => {
            let wallet_info = get_wallet_info().unwrap();
            let wallet = init_public_wallet(&wallet_info).unwrap();
            let balance = wallet.get_balance().unwrap();
            let history = wallet.list_transactions(true).unwrap();
            let unconfirmed_balance = balance.untrusted_pending + balance.trusted_pending;
            println!("x------------------------x");
            println!("Unconfirmed Balance: {:#?}", unconfirmed_balance);
            println!("Confirmed Balance: {:#?}", balance.confirmed);
            println!("x------------------------x");
            println!("Transactions: {:#?}", history.len());
            println!("x------------------------x");
            for tx in history {
                println!("Txid: {:#?}", tx.txid);
                println!("Sent: {:#?}", tx.sent);
                println!("Received: {:#?}", tx.received);
            }
            println!("x------------------------x");
        }
        Some(("receive", _)) => {
            let wallet_info = get_wallet_info().unwrap();
            let wallet = init_public_wallet(&wallet_info).unwrap();
            println!("How to recieve?");
            println!("0. Onchain");
            println!("1. Lightning");
            println!("Select 0/1 (default 0): ");
            let mut confirmation = String::new();
            std::io::stdin()
                .read_line(&mut confirmation)
                .expect("Failed to read line");
            if confirmation.trim() == "1" {
                println!("Getting invoice from boltz");
                println!("Enter amount to receive in BTC: ");
                let mut amount = String::new();
                std::io::stdin()
                    .read_line(&mut amount)
                    .expect("Failed to read line");
                let out_amount = amount.parse::<u64>().unwrap();
                // construct SwapScript
                match create_reverse_submarine_swap(&out_amount, &wallet_info) {
                    Ok((invoice, id, rev_script, keypair, preimage)) => {
                        println!("Complete payment of LN to :{}", invoice);
                        // wait till someone pays the invoice
                        let mut status = check_swap_status(&id, &rev_script).unwrap();
                        let start = Instant::now();
                        loop {
                            if status {
                                println!("Received payment from boltz");

                                let network_config = ElectrumConfig::default_bitcoin();

                                println!("Enter Return Address for payment");
                                let mut return_address = String::new();
                                std::io::stdin()
                                    .read_line(&mut confirmation)
                                    .expect("Failed to read line");
                                // Create SwapTx
                                let absolute_fees = 300;
                                let mut rv_claim_tx = BtcSwapTx::new_claim(
                                    rev_script,
                                    return_address,
                                    network_config.network(),
                                )
                                .unwrap();
                                let _ = rv_claim_tx.fetch_utxo(out_amount, network_config.clone());
                                //drain to wallet.get_address()
                                let signed_tx =
                                    rv_claim_tx.drain(keypair, preimage, absolute_fees).unwrap();
                                let txid =
                                    rv_claim_tx.broadcast(signed_tx, network_config).unwrap();
                                println!("{}", txid);

                                break;
                            } else if start.elapsed() > Duration::from_secs(60) {
                                println!("Timed out waiting for payment. Invoice is no longer valid. DO NOT PAY.");
                                break;
                            } else {
                                eprintln!("No payment yet...");
                                thread::sleep(Duration::from_secs(10));
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error creating reverse swap: {}", e);
                    }
                };
            } else {
                let address = wallet.get_address(LastUnused).unwrap();
                println!("{:#?}", address.address.to_string());
            }
        }
        Some(("send", _)) => {
            let wallet_info = get_wallet_info().unwrap();
            let wallet = init_secret_wallet(&wallet_info).unwrap();

            // ask user to paste address or invoice
            println!("Enter an address or invoice: ");
            let mut payment_info = String::new();
            std::io::stdin()
                .read_line(&mut payment_info)
                .expect("Failed to read input");

            // check if address;
            match Address::from_str(&payment_info) {
                Ok(address) => {
                    println!("Resolved input to address. Paying...");
                    // make payment:
                    println!("Enter amount in BTC: ");
                    let mut amount = String::new();
                    std::io::stdin()
                        .read_line(&mut amount)
                        .expect("Failed to read line");
                    let btc_amount = amount.parse::<f64>().unwrap();
                    let electrum_url = wallet_info.electrum_url;
                    match (send_btc(&wallet, &address, btc_amount, electrum_url)) {
                        Ok(transaction) => {
                            println!("Payment successful: {:#?}", transaction);
                        }
                        Err(e) => {
                            eprintln!("Error in payment: {}", e)
                        }
                    };
                }
                Err(e) => {
                    println!("Could not resolve input to address. Checking invoice...");
                    //check if invoice:
                    match Bolt11Invoice::from_str(&payment_info) {
                        Ok(invoice) => {
                            println!("Resolved input to invoice. Paying...");
                            //do submarine-swap

                            match create_submarine_swap(&invoice.to_string(), &wallet_info) {
                                Ok((funding_address, funding_amount)) => {
                                    let funding_amount = funding_amount as f64;
                                    //fund swap

                                    match send_btc(
                                        &wallet,
                                        &Address::from_str(&funding_address).unwrap(),
                                        funding_amount,
                                        wallet_info.electrum_url,
                                    ) {
                                        Ok(transaction) => {
                                            println!("Swap Funded: {:#?}", transaction);
                                            println!("Invoice will be paid after 1 conf.")
                                            //check if boltz paid LN addr?
                                        }
                                        Err(e) => {
                                            eprintln!("Error funding swap: {}", e);
                                        }
                                    };
                                }
                                Err(e) => {
                                    eprintln!("Error creating submarine swap: {}", e)
                                }
                            };
                        }
                        Err(e) => {
                            println!("Could not resolve input to invoice");
                            return;
                        }
                    }
                }
            }
        }
        None => {
            println!("COULD NOT FIND MATCHES. Try swappy help.")
        }
        _ => {
            println!("COULD NOT FIND MATCHES. Try swappy help.")
        }
    }
}

fn get_wallet_info() -> Result<NetworkInfoModel, String> {
    let mut root_path: PathBuf = match std::env::var("HOME") {
        Ok(home_path) => {
            let mut full_path = PathBuf::from(home_path);
            full_path.push(SWAPPY_DIR);
            full_path
        }
        Err(e) => {
            return Err(e.to_string());
        }
    };
    let wallet_info = read_db(&root_path)?;
    Ok(wallet_info)
}
fn init_public_wallet(wallet_info: &NetworkInfoModel) -> Result<Wallet<SqliteDatabase>, String> {
    let descriptors = Descriptors::new_public(&wallet_info.display_secret())?;
    let sqlite_path: PathBuf = match std::env::var("HOME") {
        Ok(home_path) => {
            let mut full_path = PathBuf::from(home_path);
            full_path.push("bdk");
            full_path
        }
        Err(e) => {
            return Err(e.to_string());
        }
    };
    create_wallet(descriptors, &sqlite_path)
}
fn init_secret_wallet(wallet_info: &NetworkInfoModel) -> Result<Wallet<SqliteDatabase>, String> {
    let descriptors = Descriptors::new_secret(&wallet_info.display_secret())?;
    let sqlite_path: PathBuf = match std::env::var("HOME") {
        Ok(home_path) => {
            let mut full_path = PathBuf::from(home_path);
            full_path.push("bdk");
            full_path
        }
        Err(e) => {
            return Err(e.to_string());
        }
    };
    create_wallet(descriptors, &sqlite_path)
}

fn send_btc(
    wallet: &Wallet<bdk::database::SqliteDatabase>,
    to_address: &Address,
    amount_btc: f64,
    electrum_url: String,
) -> Result<Transaction, String> {
    let amount_sat = Amount::from_btc(amount_btc).unwrap();

    let mut tx_builder = wallet.build_tx();
    tx_builder
        .add_recipient(to_address.script_pubkey(), amount_sat.to_sat())
        .enable_rbf()
        .fee_rate(FeeRate::from_sat_per_vb(5.0)); // Example fee rate, adjust as necessary

    let (mut psbt, details) = tx_builder.finish().unwrap();

    // Output the transaction details
    println!("{:#?}", details);

    // Sign the PSBT
    wallet.sign(&mut psbt, SignOptions::default()).unwrap();

    // Extract and broadcast the transaction
    let tx = psbt.extract_tx();
    // Broadcast the transaction using the Electrum client

    let client = Client::new(&electrum_url).unwrap();
    let blockchain = ElectrumBlockchain::from(client);
    blockchain.broadcast(&tx).unwrap();
    return Ok(tx.clone());
}

fn create_submarine_swap(
    invoice_str: &str,
    network_info: &NetworkInfoModel,
) -> Result<(String, u64), String> {
    // ensure the payment hash is the one boltz uses in their swap script
    // SECRETS
    let mnemonic = network_info.display_secret();

    let keypair =
        SwapKey::from_submarine_account(&mnemonic.to_string(), "", Chain::BitcoinTestnet, 1)
            .unwrap()
            .keypair;
    println!(
        "****SECRETS****:\n sec: {:?}, pub: {:?}",
        keypair.display_secret(),
        keypair.public_key()
    );
    // SECRETS
    let network_config = ElectrumConfig::default_bitcoin();
    let _electrum_client = network_config.build_client().unwrap();

    // CHECK FEES AND LIMITS IN BOLTZ AND MAKE SURE USER CONFIRMS THIS FIRST
    let boltz_client = BoltzApiClient::new(BOLTZ_TESTNET_URL);
    let boltz_pairs = boltz_client.get_pairs().unwrap();
    let pair_hash = boltz_pairs
        .pairs
        .pairs
        .get("BTC/BTC")
        .map(|pair_info| pair_info.hash.clone())
        .unwrap();

    let request = CreateSwapRequest::new_btc_submarine(
        pair_hash,
        invoice_str.to_string(),
        keypair.public_key().to_string().clone(),
    );
    let response = boltz_client.create_swap(request);
    let preimage_states = Preimage::from_invoice_str(invoice_str).unwrap();

    assert!(response
        .as_ref()
        .unwrap()
        .validate_script_preimage160(preimage_states.clone().hash160));

    println!("{:?}", response);
    assert!(response.is_ok());

    let timeout = response
        .as_ref()
        .unwrap()
        .timeout_block_height
        .unwrap()
        .clone();
    let _id = response.as_ref().unwrap().id.as_str();
    let funding_address = response.as_ref().unwrap().address.clone().unwrap();
    let redeem_script_string = response
        .as_ref()
        .unwrap()
        .redeem_script
        .as_ref()
        .unwrap()
        .clone();
    //funding_amount is u64. shouldn't it be f64 to repressent 0.00001 BTC?
    let funding_amount = response
        .as_ref()
        .unwrap()
        .expected_amount
        .as_ref()
        .unwrap()
        .clone();

    let boltz_script = BtcSwapScript::submarine_from_str(&redeem_script_string).unwrap();

    let constructed_script = BtcSwapScript::new(
        SwapType::Submarine,
        preimage_states.hash160.to_string(),
        boltz_script.reciever_pubkey.clone(),
        timeout as u32,
        keypair.public_key().to_string().clone(),
    );

    println!("{:?}", boltz_script);

    assert_eq!(boltz_script, constructed_script);

    println!("{}", funding_address);
    println!("{}", funding_amount);
    return Ok((funding_address, funding_amount));
}

fn create_reverse_submarine_swap(
    out_amount: &u64,
    walletInfo: &NetworkInfoModel,
) -> Result<(String, String, BtcSwapScript, KeyPair, Preimage), String> {
    // returns invoice to get paid in receive
    // const RETURN_ADDRESS: &str = "tb1qq20a7gqewc0un9mxxlqyqwn7ut7zjrj9y3d0mu";

    // SECRETS
    let mnemonic = walletInfo.display_secret();

    let keypair =
        SwapKey::from_reverse_account(&&mnemonic.to_string(), "", Chain::BitcoinTestnet, 1)
            .unwrap()
            .keypair;
    println!(
        "****SECRETS****:\n sec: {:?}, pub: {:?}",
        keypair.display_secret(),
        keypair.public_key()
    );
    let preimage = Preimage::new();
    println!(
        "****SECRETS****:\n preimage: {:?}",
        preimage.to_string().clone()
    );
    // SECRETS

    let network_config = ElectrumConfig::default_bitcoin();

    // CHECK FEES AND LIMITS IN BOLTZ AND MAKE SURE USER CONFIRMS THIS FIRST
    let boltz_client = BoltzApiClient::new(BOLTZ_TESTNET_URL);
    let boltz_pairs = boltz_client.get_pairs().unwrap();
    let pair_hash = boltz_pairs
        .pairs
        .pairs
        .get("BTC/BTC")
        .map(|pair_info| pair_info.hash.clone())
        .unwrap();

    let request = CreateSwapRequest::new_btc_reverse(
        pair_hash,
        preimage.clone().sha256.to_string(),
        keypair.public_key().to_string().clone(),
        // timeout as u64,
        out_amount.clone(),
    );
    let response = boltz_client.create_swap(request);
    println!("{:?}", response);
    assert!(response.is_ok());
    assert!(response
        .as_ref()
        .unwrap()
        .validate_invoice_preimage256(preimage.clone().sha256));

    let timeout = response
        .as_ref()
        .unwrap()
        .timeout_block_height
        .unwrap()
        .clone();
    let id = response.as_ref().unwrap().id.as_str();
    let invoice = response.as_ref().unwrap().invoice.clone().unwrap();
    let lockup_address = response.as_ref().unwrap().lockup_address.clone().unwrap();
    let redeem_script_string = response
        .as_ref()
        .unwrap()
        .redeem_script
        .as_ref()
        .unwrap()
        .clone();

    let boltz_rev_script = BtcSwapScript::reverse_from_str(&redeem_script_string).unwrap();

    let constructed_rev_script = BtcSwapScript::new(
        SwapType::ReverseSubmarine,
        preimage.hash160.to_string(),
        keypair.public_key().to_string().clone(),
        timeout as u32,
        boltz_rev_script.sender_pubkey.clone(),
    );

    assert_eq!(constructed_rev_script, boltz_rev_script);

    let constructed_address = constructed_rev_script
        .to_address(network_config.network())
        .unwrap();
    println!("{}", constructed_address.to_string());
    assert_eq!(constructed_address.to_string(), lockup_address);

    let script_balance = constructed_rev_script
        .get_balance(network_config.clone())
        .unwrap();
    assert_eq!(script_balance.0, 0);
    assert_eq!(script_balance.1, 0);
    return Ok((
        invoice,
        id.to_string(),
        constructed_rev_script,
        keypair,
        preimage,
    ));
}

fn check_swap_status(id: &String, rev_script: &BtcSwapScript) -> Result<bool, String> {
    let boltz_client = BoltzApiClient::new(BOLTZ_TESTNET_URL);
    let network_config = ElectrumConfig::default_bitcoin();

    loop {
        let request = SwapStatusRequest { id: id.to_string() };
        let response = boltz_client.swap_status(request);
        assert!(response.is_ok());
        let swap_status = response.unwrap().status;
        println!("SwapStatus: {}", swap_status);

        if swap_status == "swap.created" {
            println!("Your turn: Pay the invoice");
        }
        if swap_status == "transaction.mempool" || swap_status == "transaction.confirmed" {
            println!("*******BOLTZ******************");
            println!("*******ONCHAIN-TX*************");
            println!("*******DETECTED***************");
            let script_balance = rev_script.get_balance(network_config.clone()).unwrap();
            println!(
                "confirmed: {}, unconfirmed: {}",
                script_balance.0, script_balance.1
            );
            return Ok(true);
        }
    }
}

fn build_and_claim_tx() -> () {}

// then create script and tx
// drain funds into local wallet
