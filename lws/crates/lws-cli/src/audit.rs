use serde::Serialize;

use lws_core::Config;
use std::fs::{self, OpenOptions};
use std::io::Write;

#[derive(Debug, Serialize)]
pub struct AuditEntry {
    pub timestamp: String,
    pub wallet_id: String,
    pub operation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// Append an audit entry to the audit log.
/// Creates the log directory and file if they don't exist.
/// Silently ignores write failures (audit should not break operations).
pub fn log_audit(entry: &AuditEntry) {
    let config = Config::default();
    let log_dir = config.vault_path.join("logs");
    let log_path = log_dir.join("audit.jsonl");

    let _ = fs::create_dir_all(&log_dir);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&log_dir, fs::Permissions::from_mode(0o700));
    }

    if let Ok(json) = serde_json::to_string(entry) {
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            let _ = writeln!(file, "{}", json);

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&log_path, fs::Permissions::from_mode(0o600));
            }
        }
    }
}

/// Convenience: log a wallet creation event.
pub fn log_wallet_created(wallet_id: &str, chain_id: &str, address: &str) {
    log_audit(&AuditEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        wallet_id: wallet_id.to_string(),
        operation: "create_wallet".to_string(),
        chain_id: Some(chain_id.to_string()),
        address: Some(address.to_string()),
        details: None,
    });
}

/// Convenience: log a signing event.
pub fn log_sign(wallet_id: &str, chain_id: &str, operation: &str) {
    log_audit(&AuditEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        wallet_id: wallet_id.to_string(),
        operation: operation.to_string(),
        chain_id: Some(chain_id.to_string()),
        address: None,
        details: None,
    });
}

/// Convenience: log a wallet import event.
pub fn log_wallet_imported(wallet_id: &str, chain_id: &str, address: &str) {
    log_audit(&AuditEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        wallet_id: wallet_id.to_string(),
        operation: "import_wallet".to_string(),
        chain_id: Some(chain_id.to_string()),
        address: Some(address.to_string()),
        details: None,
    });
}

/// Convenience: log a wallet export event.
pub fn log_wallet_exported(wallet_id: &str) {
    log_audit(&AuditEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        wallet_id: wallet_id.to_string(),
        operation: "export_wallet".to_string(),
        chain_id: None,
        address: None,
        details: None,
    });
}

/// Convenience: log a wallet deletion event.
pub fn log_wallet_deleted(wallet_id: &str, name: &str) {
    log_audit(&AuditEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        wallet_id: wallet_id.to_string(),
        operation: "delete_wallet".to_string(),
        chain_id: None,
        address: None,
        details: Some(format!("name={name}")),
    });
}

/// Convenience: log a wallet rename event.
pub fn log_wallet_renamed(wallet_id: &str, old_name: &str, new_name: &str) {
    log_audit(&AuditEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        wallet_id: wallet_id.to_string(),
        operation: "rename_wallet".to_string(),
        chain_id: None,
        address: None,
        details: Some(format!("{old_name} -> {new_name}")),
    });
}

/// Convenience: log a broadcast event.
pub fn log_broadcast(wallet_id: &str, chain_id: &str, tx_hash: &str) {
    log_audit(&AuditEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        wallet_id: wallet_id.to_string(),
        operation: "broadcast_transaction".to_string(),
        chain_id: Some(chain_id.to_string()),
        address: None,
        details: Some(format!("tx_hash={tx_hash}")),
    });
}

/// Convenience: log a derive event.
pub fn log_derive(chain_id: &str, address: &str) {
    log_audit(&AuditEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        wallet_id: "ephemeral".to_string(),
        operation: "derive_address".to_string(),
        chain_id: Some(chain_id.to_string()),
        address: Some(address.to_string()),
        details: None,
    });
}
