use clap::{error::Result, ArgMatches};

#[derive(Debug)]
pub struct WalletInfoModel {
    pub electrum_url: String,
    pub boltz_url: String,
}

impl WalletInfoModel {
    pub fn from_arg_matches(am: ArgMatches) -> Self {
        let electrum = am.get_one::<String>("electrum").unwrap();
        let boltz = am.get_one::<String>("boltz").unwrap();
        WalletInfoModel {
            electrum_url: electrum.to_string(),
            boltz_url: boltz.to_string(),
        }
    }
}

pub fn create_db(wallet_info: WalletInfoModel, path: &str) {
    // check if db already exists; if true return "Db Exists"; else continue
    // create wallet
    // show user mnemonic
    // ask user to confirm that backup is complete
    // then do the below
    let db = sled::open(path).unwrap();
    // as in fs::open
    db.insert(b"electrum", wallet_info.electrum_url.as_bytes())
        .unwrap();
    db.insert(b"boltz", wallet_info.boltz_url.as_bytes())
        .unwrap();
    // insert wallet data (mnemonic and public descriptor)
    drop(db);
    println!("Written to db.");
    ()
}

pub fn read_db(path: &str) -> Result<WalletInfoModel, String> {
    let db = sled::open(path).unwrap();
    let value = db.get("electrum").unwrap().unwrap();
    let electrum = std::str::from_utf8(&value).unwrap();
    let value = db.get("boltz").unwrap().unwrap();
    let boltz = std::str::from_utf8(&value).unwrap();
    println!("Electrum Url: {}", electrum);
    println!("Boltz Url: {}", boltz);
    Ok(WalletInfoModel {
        electrum_url: electrum.to_string(),
        boltz_url: boltz.to_string(),
    })
}
