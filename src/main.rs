mod db;
mod util;
mod wallet;

use clap::{Arg, ArgMatches, Command};
use db::{create_db, read_db, WalletInfoModel};
use wallet::key;
fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");
    let path = "/home/anorak/.swappy";

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
            Command::new("read")
                .about("read wallet & network settings ")
                .display_order(2),
        )
        .get_matches();

    match api.subcommand() {
        Some(("create", arg_matches)) => {
            let wallet_info = WalletInfoModel::from_arg_matches(arg_matches.clone());
            create_db(wallet_info, path);
        }
        Some(("read", _)) => {
            let wallet_info = read_db(path).unwrap();
            println!("{:?}", wallet_info)
        }
        None => {
            println!("COULD NOT FIND MATCHES. Try swappy help.")
        }
        _ => {
            println!("COULD NOT FIND MATCHES. Try swappy help.")
        }
    }
}
