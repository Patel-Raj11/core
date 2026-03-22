use crate::commands::read_passphrase;
use crate::CliError;

/// Concrete WalletAccess backed by ows-lib.
struct OwsLibWallet {
    wallet_name: String,
    passphrase: String,
}

impl ows_pay::WalletAccess for OwsLibWallet {
    fn evm_account(&self) -> Result<ows_pay::EvmAccount, ows_pay::PayError> {
        let info = ows_lib::get_wallet(&self.wallet_name, None).map_err(|e| {
            ows_pay::PayError::new(ows_pay::PayErrorCode::WalletNotFound, e.to_string())
        })?;
        let acct = info
            .accounts
            .iter()
            .find(|a| a.chain_id.starts_with("eip155:"))
            .ok_or_else(|| {
                ows_pay::PayError::new(ows_pay::PayErrorCode::WalletNotFound, "no EVM account")
            })?;
        Ok(ows_pay::EvmAccount {
            address: acct.address.clone(),
        })
    }

    fn sign_typed_data(
        &self,
        chain: &str,
        typed_data_json: &str,
    ) -> Result<ows_pay::TypedDataSignature, ows_pay::PayError> {
        let result = ows_lib::sign_typed_data(
            &self.wallet_name,
            chain,
            typed_data_json,
            Some(&self.passphrase),
            None,
            None,
        )
        .map_err(|e| ows_pay::PayError::new(ows_pay::PayErrorCode::SigningFailed, e.to_string()))?;
        Ok(ows_pay::TypedDataSignature {
            signature: format!("0x{}", result.signature),
        })
    }
}

/// `ows pay request <url> --wallet <name> [--method GET] [--body '{}']`
pub fn run(
    url: &str,
    wallet_name: &str,
    method: &str,
    body: Option<&str>,
    skip_passphrase: bool,
) -> Result<(), CliError> {
    let passphrase = if skip_passphrase {
        String::new()
    } else {
        read_passphrase().to_string()
    };

    let wallet = OwsLibWallet {
        wallet_name: wallet_name.to_string(),
        passphrase,
    };

    let rt =
        tokio::runtime::Runtime::new().map_err(|e| CliError::InvalidArgs(format!("tokio: {e}")))?;

    let result = rt.block_on(ows_pay::pay(&wallet, url, method, body))?;

    if let Some(ref payment) = result.payment {
        if !payment.amount.is_empty() {
            eprintln!(
                "Paid {} on {} via {}",
                payment.amount, payment.network, result.protocol
            );
        } else {
            eprintln!("Paid via {}", result.protocol);
        }
    }

    if result.status >= 400 {
        eprintln!("HTTP {}", result.status);
    }

    println!("{}", result.body);
    Ok(())
}

/// `ows pay discover [--query <search>] [--limit N] [--offset N]`
pub fn discover(
    query: Option<&str>,
    limit: Option<u64>,
    offset: Option<u64>,
) -> Result<(), CliError> {
    let rt =
        tokio::runtime::Runtime::new().map_err(|e| CliError::InvalidArgs(format!("tokio: {e}")))?;

    let result = rt.block_on(ows_pay::discover(query, limit, offset))?;

    if result.services.is_empty() {
        eprintln!("No services found.");
        return Ok(());
    }

    eprintln!(
        "Showing {}-{} of {} services:\n",
        result.offset + 1,
        result.offset + result.services.len() as u64,
        result.total,
    );
    for svc in &result.services {
        println!(
            "  {:>8}  {:<8}  {}",
            svc.price, svc.network, svc.description
        );
        println!("  {:>8}  {:8}  {}", "", "", svc.url);
        println!();
    }

    Ok(())
}
