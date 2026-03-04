use std::io::IsTerminal;

use lws_core::{ChainType, EncryptedWallet, KeyType, WalletAccount};
use lws_signer::{decrypt, encrypt, signer_for_chain, CryptoEnvelope, HdDeriver, Mnemonic, MnemonicStrength};

use crate::audit;
use crate::vault;
use crate::{parse_chain, CliError};

/// Returns a default CAIP-2 chain reference for a given chain type.
fn default_chain_reference(chain: ChainType) -> &'static str {
    match chain {
        ChainType::Evm => "1",
        ChainType::Solana => "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp",
        ChainType::Bitcoin => "000000000019d6689c085ae165831e93",
        ChainType::Cosmos => "cosmoshub-4",
        ChainType::Tron => "mainnet",
    }
}

pub fn create(name: &str, chain_str: &str, words: u32, show_mnemonic: bool) -> Result<(), CliError> {
    let chain = parse_chain(chain_str)?;
    let strength = match words {
        12 => MnemonicStrength::Words12,
        24 => MnemonicStrength::Words24,
        _ => return Err(CliError::InvalidArgs("--words must be 12 or 24".into())),
    };

    // Generate mnemonic
    let mnemonic = Mnemonic::generate(strength)?;
    let signer = signer_for_chain(chain);
    let path = signer.default_derivation_path(0);
    let curve = signer.curve();

    let key = HdDeriver::derive_from_mnemonic(&mnemonic, "", &path, curve)?;
    let address = signer.derive_address(key.expose())?;

    let chain_id_str = format!("{}:{}", chain.namespace(), default_chain_reference(chain));
    let account_id_str = format!("{chain_id_str}:{address}");

    // Encrypt the mnemonic entropy
    let phrase = mnemonic.phrase();
    let crypto_envelope = encrypt(phrase.expose(), "")?;
    let crypto_json = serde_json::to_value(&crypto_envelope)?;

    let wallet_id = uuid::Uuid::new_v4().to_string();

    let wallet = EncryptedWallet::new(
        wallet_id.clone(),
        name.to_string(),
        chain,
        vec![WalletAccount {
            account_id: account_id_str,
            address: address.clone(),
            chain_id: chain_id_str.clone(),
            derivation_path: path.clone(),
        }],
        crypto_json,
        KeyType::Mnemonic,
    );

    vault::save_encrypted_wallet(&wallet)?;

    // Audit log
    audit::log_wallet_created(&wallet_id, &chain_id_str, &address);

    println!("Wallet created: {wallet_id}");
    println!("Name:           {name}");
    println!("Chain:          {chain}");
    println!("Address:        {address}");
    println!("Path:           {path}");

    if show_mnemonic {
        let phrase_str = String::from_utf8(phrase.expose().to_vec())
            .map_err(|e| CliError::InvalidArgs(format!("invalid UTF-8 in mnemonic: {e}")))?;
        eprintln!();
        eprintln!("⚠️  WARNING: The mnemonic below provides FULL ACCESS to this wallet.");
        eprintln!("⚠️  Store it securely offline. It will NOT be shown again.");
        eprintln!();
        println!("{phrase_str}");
    } else {
        eprintln!();
        eprintln!("Mnemonic encrypted and saved to vault.");
        eprintln!("Use --show-mnemonic at creation time if you need a backup copy.");
    }

    Ok(())
}

pub fn import(
    name: &str,
    chain_str: &str,
    use_mnemonic: bool,
    use_private_key: bool,
    index: u32,
) -> Result<(), CliError> {
    if use_mnemonic == use_private_key {
        return Err(CliError::InvalidArgs(
            "specify exactly one of --mnemonic or --private-key".into(),
        ));
    }

    let chain = parse_chain(chain_str)?;
    let signer = signer_for_chain(chain);
    let (address, secret_bytes, key_type, path) = if use_mnemonic {
        let phrase = super::read_mnemonic()?;
        let mnemonic = Mnemonic::from_phrase(&phrase)?;
        let deriv_path = signer.default_derivation_path(index);
        let curve = signer.curve();
        let key = HdDeriver::derive_from_mnemonic(&mnemonic, "", &deriv_path, curve)?;
        let addr = signer.derive_address(key.expose())?;
        let phrase_bytes = mnemonic.phrase();
        (addr, phrase_bytes.expose().to_vec(), KeyType::Mnemonic, deriv_path)
    } else {
        let hex_key = super::read_private_key()?;
        let hex_trimmed = hex_key.strip_prefix("0x").unwrap_or(&hex_key);
        let key_bytes = hex::decode(hex_trimmed)
            .map_err(|e| CliError::InvalidArgs(format!("invalid hex private key: {e}")))?;
        let addr = signer.derive_address(&key_bytes)?;
        let path = String::new();
        (addr, key_bytes, KeyType::PrivateKey, path)
    };

    let crypto_envelope = encrypt(&secret_bytes, "")?;
    let crypto_json = serde_json::to_value(&crypto_envelope)?;

    let wallet_id = uuid::Uuid::new_v4().to_string();
    let chain_id_str = format!("{}:{}", chain.namespace(), default_chain_reference(chain));
    let account_id_str = format!("{chain_id_str}:{address}");

    let wallet = EncryptedWallet::new(
        wallet_id.clone(),
        name.to_string(),
        chain,
        vec![WalletAccount {
            account_id: account_id_str,
            address: address.clone(),
            chain_id: chain_id_str.clone(),
            derivation_path: path.clone(),
        }],
        crypto_json,
        key_type,
    );

    vault::save_encrypted_wallet(&wallet)?;
    audit::log_wallet_imported(&wallet_id, &chain_id_str, &address);

    println!("Wallet imported: {wallet_id}");
    println!("Name:            {name}");
    println!("Chain:           {chain}");
    println!("Address:         {address}");
    if !path.is_empty() {
        println!("Path:            {path}");
    }

    Ok(())
}

pub fn export(wallet_name: &str) -> Result<(), CliError> {
    if !std::io::stdin().is_terminal() {
        return Err(CliError::InvalidArgs(
            "wallet export requires an interactive terminal (do not pipe stdin)".into(),
        ));
    }

    let wallet = vault::load_wallet_by_name_or_id(wallet_name)?;
    let envelope: CryptoEnvelope = serde_json::from_value(wallet.crypto.clone())?;
    let secret = decrypt(&envelope, "")?;

    let secret_str = String::from_utf8(secret.expose().to_vec())
        .map_err(|_| CliError::InvalidArgs("wallet contains invalid UTF-8 secret".into()))?;

    match wallet.key_type {
        KeyType::Mnemonic => {
            eprintln!();
            eprintln!("WARNING: The mnemonic below provides FULL ACCESS to this wallet.");
            eprintln!("Do not share it. Store it securely offline.");
            eprintln!();
            println!("{secret_str}");
        }
        KeyType::PrivateKey => {
            eprintln!();
            eprintln!("WARNING: The private key below provides FULL ACCESS to this wallet.");
            eprintln!("Do not share it. Store it securely offline.");
            eprintln!();
            println!("{secret_str}");
        }
    }

    audit::log_wallet_exported(&wallet.id);
    Ok(())
}

pub fn delete(wallet_name: &str, confirm: bool) -> Result<(), CliError> {
    if !confirm {
        eprintln!("To delete a wallet, pass --confirm.");
        eprintln!("Consider exporting it first: lws wallet export --wallet {wallet_name}");
        return Err(CliError::InvalidArgs(
            "--confirm is required to delete a wallet".into(),
        ));
    }

    let wallet = vault::load_wallet_by_name_or_id(wallet_name)?;
    let id = wallet.id.clone();
    let name = wallet.name.clone();

    vault::delete_wallet(&id)?;
    audit::log_wallet_deleted(&id, &name);

    println!("Wallet deleted: {id} ({name})");
    Ok(())
}

pub fn rename(wallet_name: &str, new_name: &str) -> Result<(), CliError> {
    let mut wallet = vault::load_wallet_by_name_or_id(wallet_name)?;

    if wallet.name == new_name {
        return Ok(());
    }

    if vault::wallet_name_exists(new_name)? {
        return Err(CliError::InvalidArgs(format!(
            "a wallet named '{new_name}' already exists"
        )));
    }

    let old_name = wallet.name.clone();
    wallet.name = new_name.to_string();
    vault::save_encrypted_wallet(&wallet)?;
    audit::log_wallet_renamed(&wallet.id, &old_name, new_name);

    println!("Wallet renamed: '{}' -> '{}'", old_name, new_name);
    Ok(())
}

pub fn list() -> Result<(), CliError> {
    let wallets = vault::list_encrypted_wallets()?;

    if wallets.is_empty() {
        println!("No wallets found.");
        return Ok(());
    }

    for w in &wallets {
        println!("ID:      {}", w.id);
        println!("Name:    {}", w.name);
        println!("Chain:   {}", w.chain_type);
        println!("Secured: ✓ (encrypted)");
        for acct in &w.accounts {
            println!("  {} → {}", acct.chain_id, acct.address);
        }
        println!("Created: {}", w.created_at);
        println!();
    }

    Ok(())
}
