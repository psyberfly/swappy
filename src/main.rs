mod db;
mod util;
mod wallet;
use clap::{Arg, Command};
use db::{create_db, read_db, NetworkInfoModel, WalletInfoModel};
use lightning_invoice::{payment, Bolt11Invoice};
use std::path::{Path, PathBuf};
use wallet::util::{create_wallet, Descriptors};
const SWAPPY_DIR: &str = ".swappy";
use bdk::{
    bitcoin::{Address, Network, Transaction},
    blockchain::ElectrumBlockchain,
    database::SqliteDatabase,
    electrum_client::Client,
    wallet::AddressIndex::{LastUnused, Peek},
    SyncOptions, Wallet,
};
use std::fs::OpenOptions;
use std::io::Write;
use std::str::FromStr;
use std::thread;
use std::time::{Duration, Instant};

use crate::wallet::util::send_btc;
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
            // println!("DELETING WALLET! CAREFUL! ARE YOU SURE? Type 'yes' to confirm.");
            // let mut confirmation = String::new();
            // std::io::stdin()
            //     .read_line(&mut confirmation)
            //     .expect("Failed to read line");
            // if confirmation.trim() != "yes" {
            //     println!("Aborting delete.");
            //     return;
            // }
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
                // initialize BoltzClient and swap
                // construct SwapScript
                let mut status = false;
                let start = Instant::now();
                loop {
                    if status {
                        println!("Got payment");
                        // Create SwapTx and drain to wallet.get_address()
                        break;
                    } else if start.elapsed() > Duration::from_secs(60) {
                        println!("Timed out waiting for payment. Invoice is no longer valid. DO NOT PAY.");
                        break;
                    } else {
                        eprintln!("No payment yet...");
                        thread::sleep(Duration::from_secs(10));
                    }
                }
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
            match (Address::from_str(&payment_info)) {
                Ok(address) => {
                    println!("Resolved input to address. Paying...");
                    //if address, make payment:
                    let btc_amount = 0.00001;
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
                    match (Bolt11Invoice::from_str(&payment_info)) {
                        Ok(invoice) => {
                            println!("Resolved input to invoice. Paying...");
                            //if invoice, do submarine-swap
                        }
                        Err(e) => {
                            println!("Could not resolve input to invoice");
                            return;
                        }
                    }
                }
            }

            // println!("DELETING WALLET! CAREFUL! ARE YOU SURE? Type 'yes' to confirm.");
            // let mut confirmation = String::new();
            // std::io::stdin()
            //     .read_line(&mut confirmation)
            //     .expect("Failed to read line");
            // if confirmation.trim() != "yes" {
            //     println!("Aborting delete.");
            //     return;
            // }
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

fn create_submarine() -> () {
    // return address to pay in send
}
// immediately pay this address

fn create_reverse() -> () {
    // returns invoice to get paid in receive
}

fn check_swap_status() -> () {}

fn build_and_claim_tx() -> () {}
// wait till someone pays the invoice
// then create script and tx
// drain funds into local wallet
