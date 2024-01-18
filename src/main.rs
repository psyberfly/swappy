mod db;
mod util;
mod wallet;

use clap::{Arg, Command};
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
            // check if db already exists; if true return "Db Exists"; else continue
            // create wallet
            // show user mnemonic
            // ask user to confirm that backup is complete
            // then do the below
            let electrum = arg_matches.get_one::<String>("electrum").unwrap();
            let boltz = arg_matches.get_one::<String>("boltz").unwrap();
            let db = sled::open(path).unwrap(); // as in fs::open
            db.insert(b"electrum", electrum.as_bytes()).unwrap();          
            db.insert(b"boltz", boltz.as_bytes()).unwrap();
            // insert wallet data (mnemonic and public descriptor)
            drop(db);
            println!("Written to db.");
            ()
        },
        Some(("read", _)) => {
            let db = sled::open(path).unwrap();
            let value = db.get("electrum").unwrap().unwrap();
            let electrum= std::str::from_utf8(&value).unwrap();     
            let value = db.get("boltz").unwrap().unwrap();
            let boltz= std::str::from_utf8(&value).unwrap();     
            println!("Electrum Url: {}", electrum);
            println!("Boltz Url: {}", boltz);
            ()
        },
        None => {
            println!("COULD NOT FIND MATCHES. Try swappy help.")
        },
        _ => {
            println!("COULD NOT FIND MATCHES. Try swappy help.")
        }
    }
}
