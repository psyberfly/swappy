mod db;
mod util;
mod wallet;
use clap::{Arg, Command};
use db::{create_db, read_db, NetworkInfoModel, WalletInfoModel};
use std::path::{Path, PathBuf};
use wallet::util::{create_wallet, Descriptors};
const SWAPPY_DIR: &str = ".swappy";
use bdk::{
    bitcoin::Network,
    blockchain::ElectrumBlockchain,
    database::SqliteDatabase,
    electrum_client::Client,
    wallet::AddressIndex::{LastUnused, Peek},
    SyncOptions, Wallet,
};

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
            let descriptor = wallet::util::Descriptors::new(&mnemonic);

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
            let wallet = init_wallet(&wallet_info).unwrap();
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
            let wallet = init_wallet(&wallet_info).unwrap();
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
            let wallet = init_wallet(&wallet_info).unwrap();
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
            } else {
                let address = wallet.get_address(LastUnused).unwrap();
                println!("{:#?}", address.address.to_string());
            }
        }
        Some(("send", _)) => {
            let wallet_info = get_wallet_info().unwrap();
            let wallet = init_wallet(&wallet_info).unwrap();
            let address = wallet.get_address(LastUnused);
            println!("{:#?}", address);
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
fn init_wallet(wallet_info: &NetworkInfoModel) -> Result<Wallet<SqliteDatabase>, String> {
    let descriptors = Descriptors::new(&wallet_info.display_secret())?;
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
