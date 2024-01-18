use bdk::bitcoin::Network;
use bdk::wallet::Wallet;
use bdk::{
    database::SqliteDatabase,
    keys::{ExtendedKey, GeneratableKey},
};
use clap::{error::Result, ArgMatches};
use std::path::Path;

#[derive(Debug)]
pub struct NetworkInfoModel {
    pub network: Network,
    pub electrum_url: String,
    pub boltz_url: String,
    mnemonic: Option<String>,
}

impl NetworkInfoModel {
    pub fn from_arg_matches(am: ArgMatches) -> Self {
        let electrum = am.get_one::<String>("electrum").unwrap();
        let boltz = am.get_one::<String>("boltz").unwrap();
        NetworkInfoModel {
            network: Network::Testnet,
            electrum_url: electrum.to_string(),
            boltz_url: boltz.to_string(),
            mnemonic: None,
        }
    }
    pub fn update_mnemonic(&mut self, mnemonic: String) -> Result<&mut Self, String> //should return type be <Self>?
    {
        if self.mnemonic.is_none() {
            self.mnemonic = Some(mnemonic);
            Ok(self) //should this return self.clone() ?
        } else {
            Err("mnemonic exists.".to_string())
        }
    }
    pub fn display_secret(&self) -> String {
        if self.mnemonic.is_none() {
            "None".to_string()
        } else {
            self.mnemonic.clone().unwrap()
        }
    }
}

// fn check_db_exists(path: &Path) -> Result<bool, sled::Error> {
//     // Check if the directory already exists
//     let already_exists = path.exists();

//     // Try opening the database (this will create the directory if it doesn't exist)
//     let _db = sled::open(path)?;

//     // If the directory already existed, we assume the DB also existed
//     Ok(already_exists)
// }

pub fn create_db(wallet_info: NetworkInfoModel, path: &Path) -> Result<(), String> {
    let already_exists = path.exists();
    if already_exists {
        return Err("Wallet already exists. Retry after swappy delete.".to_string());
    }
    if wallet_info.mnemonic.is_none() {
        return Err("No mnemonic to write to db".to_string());
    }
    // Open the sled database
    let db = sled::open(path).unwrap();
    db.insert(b"electrum", wallet_info.electrum_url.as_bytes())
        .map_err(|e| e.to_string())?;
    db.insert(b"boltz", wallet_info.boltz_url.as_bytes())
        .unwrap();
    // Insert wallet data (mnemonic and public descriptor)

    db.insert(b"mnemonic", wallet_info.mnemonic.unwrap().as_bytes())
        .unwrap();
    // You may also want to store other wallet-related information

    drop(db);
    println!("Written to db.");
    Ok(())
}

pub fn read_db(path: &Path) -> Result<NetworkInfoModel, String> {
    let db = sled::open(path).unwrap();
    let value = db.get("electrum").unwrap().unwrap();
    let electrum = std::str::from_utf8(&value).unwrap();
    let value = db.get("boltz").unwrap().unwrap();
    let boltz = std::str::from_utf8(&value).unwrap();
    let value = db.get("mnemonic").unwrap().unwrap();
    let mnemonic = std::str::from_utf8(&value).unwrap();
    Ok(NetworkInfoModel {
        network: Network::Testnet,
        electrum_url: electrum.to_string(),
        boltz_url: boltz.to_string(),
        mnemonic: Some(mnemonic.to_string()),
    })
}

pub struct WalletInfoModel {
    pub mnemonic: String,
    pub network: Network,
}

fn create_wallet_db(wallet_info: WalletInfoModel) -> () {
    // Create wallet -- bdk

    // create policy -> descriptor
    // let wallet = Wallet::new(
    //     &xprv.into(),
    //     None, // Electrum not needed for offline wallet
    //     Network::Testnet,
    //     SqliteDatabase::new(path),
    // )
    // .unwrap();
}
