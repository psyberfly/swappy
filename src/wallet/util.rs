use crate::wallet::util::bdk::database::SqliteDatabase;
use crate::wallet::util::bdk::wallet::AddressIndex::{New, Peek};
use bdk::bitcoin::secp256k1::Secp256k1;
use bdk::bitcoin::util::bip32::{DerivationPath, ExtendedPrivKey, ExtendedPubKey, Fingerprint};
use bdk::bitcoin::{Address, Amount, Network, Transaction};
use bdk::blockchain::{electrum, Blockchain, ElectrumBlockchain};
use bdk::electrum_client::Client;
use bdk::keys::bip39::Mnemonic;
use bdk::keys::bip39::{Language, WordCount};
use bdk::keys::{GeneratableKey, GeneratedKey};
use bdk::miniscript::{Descriptor, DescriptorPublicKey, Segwitv0};
use bdk::{self, bitcoin, descriptor, FeeRate, SignOptions, TxBuilder};
use bdk::{SyncOptions, Wallet};
use std::path::Path;
use std::str::FromStr;

type S5Mnemonic = String;
type S5Xpub = String;

pub fn create_mnemonic() -> S5Mnemonic {
    let mnemonic: GeneratedKey<Mnemonic, Segwitv0> =
        bdk::keys::bip39::Mnemonic::generate((WordCount::Words12, Language::English)).unwrap();
    mnemonic.to_string()
}

pub fn create_xpub() -> S5Xpub {
    return "xpub".to_string();
}

#[derive(Debug)]
pub struct Descriptors {
    pub deposit: String,
    pub change: String,
}

impl Descriptors {
    pub fn new_public(xpub: &S5Xpub) -> Result<Self, String> {
        let secp = Secp256k1::new();
        let mnemonic: Result<Mnemonic, _> = Mnemonic::parse_in(Language::English, xpub);
        match mnemonic {
            Ok(mnemonic) => {
                let seed = mnemonic.to_seed("");
                let xprv = ExtendedPrivKey::new_master(Network::Testnet, &seed).unwrap();
                let fp: Fingerprint = xprv.fingerprint(&secp);
                let derivation_path = DerivationPath::from_str("m/84'/1'/0'").unwrap();
                let derived_xprv = xprv.derive_priv(&secp, &derivation_path).unwrap();
                let xpub = ExtendedPubKey::from_priv(&secp, &derived_xprv);
                let descriptor = format!(
                    "wpkh([{}/{}]{})",
                    fp,
                    derivation_path.to_string().replace("m/", ""),
                    xpub
                );
                Ok(Descriptors {
                    deposit: descriptor.replace(')', "/0/*)"),
                    change: descriptor.replace(')', "/1/*)"),
                })
            }
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn new_secret(mnemonic_str: &S5Mnemonic) -> Result<Self, String> {
        let secp = Secp256k1::new();
        let mnemonic: Result<Mnemonic, _> = Mnemonic::parse_in(Language::English, mnemonic_str);
        match mnemonic {
            Ok(mnemonic) => {
                let seed = mnemonic.to_seed("");
                let xprv = ExtendedPrivKey::new_master(Network::Testnet, &seed).unwrap();
                let fp: Fingerprint = xprv.fingerprint(&secp);
                let derivation_path = DerivationPath::from_str("m/84'/1'/0'").unwrap();
                let derived_xprv = xprv.derive_priv(&secp, &derivation_path).unwrap();
                let descriptor = format!(
                    "wpkh([{}/{}]{})",
                    fp,
                    derivation_path.to_string().replace("m/", ""),
                    derived_xprv
                );
                Ok(Descriptors {
                    deposit: descriptor.replace(')', "/0/*)"),
                    change: descriptor.replace(')', "/1/*)"),
                })
            }
            Err(e) => Err(e.to_string()),
        }
    }
}

pub fn create_wallet(
    descriptors: Descriptors,
    sqlite_path: &Path,
) -> Result<Wallet<bdk::database::SqliteDatabase>, String> {
    let wallet = Wallet::new(
        &descriptors.deposit,
        Some(&descriptors.change),
        bitcoin::Network::Testnet,
        SqliteDatabase::new(sqlite_path),
    )
    .unwrap();
    Ok(wallet)
}

pub fn send_btc(
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

#[cfg(test)]
mod test {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_create_descriptor() {
        let mnemonic = "rebel opinion faculty ticket wisdom shield ecology buyer wisdom dog fish below alcohol attack enact marriage ranch legal doll monkey sense click edit absent";
        let expected_xpub  = "[7b51f3f7/84'/1'/0']tpubDCCnk1bwtxqNaFbQstA7iGuzKkooWrZZ6HxHeEQ3dZbKCDftjW7pLGMjdwh1mKXK52SW6TYyoGjzFWaaSAVLCs7aq2Y4TZyaWgocm9GxuoQ";
        let expected_deposit_descriptor = format!("wpkh({}/0/*)", expected_xpub);
        let descriptors = Descriptors::new_public(mnemonic).unwrap();
        print!("{:#?}", descriptors);
        assert_eq!(descriptors.deposit, expected_deposit_descriptor);
    }

    #[test]
    fn test_wallet_ops() {
        let mnemonic = "rebel opinion faculty ticket wisdom shield ecology buyer wisdom dog fish below alcohol attack enact marriage ranch legal doll monkey sense click edit absent";
        let descriptors = Descriptors::new_public(mnemonic).unwrap();
        // let client = Client::new("ssl://electrum.blockstream.info:60002").unwrap();
        let sqlite_path: PathBuf = match std::env::var("HOME") {
            Ok(home_path) => {
                let mut full_path = PathBuf::from(home_path);
                full_path.push(".swappy/bdk");
                full_path
            }
            Err(e) => {
                eprintln!("Failed to get HOME path: {}", e);
                return;
            }
        };
        let wallet = create_wallet(descriptors, &sqlite_path).unwrap();
        let first_address = wallet.get_address(Peek(0));
        println!("First Address: {:#?}", first_address);
    }
}
