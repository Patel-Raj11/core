use crate::curve::Curve;
use crate::traits::{ChainSigner, SignOutput, SignerError};
use k256::ecdsa::signature::hazmat::PrehashSigner;
use k256::ecdsa::SigningKey;
use k256::PublicKey;
use ows_core::ChainType;
use ripemd::Ripemd160;
use sha2::{Digest, Sha256, Sha512};

/// XRPL chain signer (secp256k1).
///
/// Signing algorithm: SHA512-half of the encoded bytes, signed with secp256k1,
/// DER-encoded output.
///
/// The caller is responsible for encoding the transaction using the appropriate
/// `encodeFor*` function from ripple-binary-codec before passing bytes here.
/// Each encode function embeds the correct hash prefix/suffix for its signing mode
/// (single, multisig, claim, batch).
pub struct XrplSigner;

/// XRPL hash function: first 32 bytes of SHA-512.
///
/// Equivalent to `sha512Half` in ripple-binary-codec/src/hashes.ts.
fn sha512_half(data: &[u8]) -> [u8; 32] {
    let hash = Sha512::digest(data);
    hash[..32].try_into().expect("sha512 output is 64 bytes")
}

/// Sign a 32-byte digest with secp256k1, returning a DER-encoded signature.
///
/// XRPL requires DER encoding (not raw r||s). `k256` provides this via
/// `to_der()` on the `Signature` type.
fn sign_secp256k1(private_key: &[u8], digest: &[u8; 32]) -> Result<Vec<u8>, SignerError> {
    let signing_key = SigningKey::from_slice(private_key)
        .map_err(|e| SignerError::InvalidPrivateKey(e.to_string()))?;
    let sig: k256::ecdsa::Signature = signing_key
        .sign_prehash(digest)
        .map_err(|e| SignerError::SigningFailed(e.to_string()))?;
    Ok(sig.to_der().as_bytes().to_vec())
}

impl ChainSigner for XrplSigner {
    fn chain_type(&self) -> ChainType {
        ChainType::Xrpl
    }

    fn curve(&self) -> Curve {
        Curve::Secp256k1
    }

    fn coin_type(&self) -> u32 {
        144
    }

    fn default_derivation_path(&self, index: u32) -> String {
        format!("m/44'/144'/0'/0/{}", index)
    }

    /// Derive a classic XRPL `r`-address from a private key.
    ///
    /// Algorithm: compressed pubkey → SHA256 → RIPEMD160 → base58check
    /// with version byte `0x00` using the Ripple alphabet.
    ///
    /// Equivalent to `deriveAddressFromBytes` in ripple-keypairs/src/index.ts.
    fn derive_address(&self, private_key: &[u8]) -> Result<String, SignerError> {
        let signing_key = SigningKey::from_slice(private_key)
            .map_err(|e| SignerError::InvalidPrivateKey(e.to_string()))?;
        let verifying_key = signing_key.verifying_key();
        let pubkey_bytes = PublicKey::from(verifying_key).to_sec1_bytes(); // compressed, 33 bytes

        let sha256 = Sha256::digest(&pubkey_bytes);
        let account_id = Ripemd160::digest(sha256);

        // Base58Check: version byte 0x00 || 20-byte account_id
        let mut payload = Vec::with_capacity(21);
        payload.push(0x00u8);
        payload.extend_from_slice(&account_id);

        Ok(bs58::encode(payload)
            .with_alphabet(bs58::Alphabet::RIPPLE)
            .with_check()
            .into_string())
    }

    /// Sign a pre-hashed 32-byte message with secp256k1 (DER output).
    fn sign(&self, private_key: &[u8], message: &[u8]) -> Result<SignOutput, SignerError> {
        let digest: [u8; 32] = message.try_into().map_err(|_| {
            SignerError::InvalidMessage(format!(
                "expected 32-byte hash, got {} bytes",
                message.len()
            ))
        })?;
        let sig = sign_secp256k1(private_key, &digest)?;
        Ok(SignOutput {
            signature: sig,
            recovery_id: None,
            public_key: None,
        })
    }

    /// Sign the output of any `encodeFor*` function from ripple-binary-codec.
    ///
    /// Each encode function already prepends the appropriate hash prefix:
    /// - `encodeForSigning(tx)`              → `STX\0` || serialized fields
    /// - `encodeForMultisigning(tx, account)` → `SMT\0` || serialized fields || account_id
    /// - `encodeForSigningClaim(claim)`       → `CLM\0` || channel_id || amount
    /// - `encodeForSigningBatch(batch)`       → `BCH\0` (0x42434800) || batch fields
    ///
    /// Pass the hex-decoded bytes of that output directly here. OWS computes
    /// SHA512-half and signs with secp256k1, returning a DER-encoded signature.
    fn sign_transaction(
        &self,
        private_key: &[u8],
        tx_bytes: &[u8],
    ) -> Result<SignOutput, SignerError> {
        if tx_bytes.is_empty() {
            return Err(SignerError::InvalidTransaction(
                "transaction bytes must not be empty".into(),
            ));
        }

        let digest = sha512_half(tx_bytes);
        let sig = sign_secp256k1(private_key, &digest)?;
        Ok(SignOutput {
            signature: sig,
            recovery_id: None,
            public_key: None,
        })
    }

    /// Off-chain message signing is not yet supported for XRPL.
    ///
    /// XRPL has no canonical message signing standard equivalent to EIP-191.
    /// A convention must be defined before this can be implemented.
    fn sign_message(
        &self,
        _private_key: &[u8],
        _message: &[u8],
    ) -> Result<SignOutput, SignerError> {
        Err(SignerError::SigningFailed(
            "XRPL off-chain message signing is not supported: no canonical standard exists. \
             Define a convention (e.g. SHA512Half(XMSG\\0 || message)) before enabling this."
                .into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hd::HdDeriver;
    use crate::mnemonic::Mnemonic;

    const ABANDON_PHRASE: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    /// Known test private key (32 bytes).
    fn test_privkey() -> Vec<u8> {
        // Derived from abandon mnemonic at m/44'/144'/0'/0/0 with secp256k1
        // via xrpl.js: Wallet.fromMnemonic(ABANDON_PHRASE, { derivationPath: "m/44'/144'/0'/0/0", algorithm: ECDSA.secp256k1 })
        let mnemonic = Mnemonic::from_phrase(ABANDON_PHRASE).unwrap();
        let signer = XrplSigner;
        let path = signer.default_derivation_path(0);
        HdDeriver::derive_from_mnemonic(&mnemonic, "", &path, Curve::Secp256k1)
            .unwrap()
            .expose()
            .to_vec()
    }

    #[test]
    fn test_chain_properties() {
        let signer = XrplSigner;
        assert_eq!(signer.chain_type(), ChainType::Xrpl);
        assert_eq!(signer.curve(), Curve::Secp256k1);
        assert_eq!(signer.coin_type(), 144);
    }

    #[test]
    fn test_derivation_path() {
        let signer = XrplSigner;
        assert_eq!(signer.default_derivation_path(0), "m/44'/144'/0'/0/0");
        assert_eq!(signer.default_derivation_path(1), "m/44'/144'/0'/0/1");
        assert_eq!(signer.default_derivation_path(5), "m/44'/144'/0'/0/5");
    }

    #[test]
    fn test_derive_address_format() {
        let privkey = test_privkey();
        let signer = XrplSigner;
        let address = signer.derive_address(&privkey).unwrap();

        assert!(
            address.starts_with('r'),
            "XRPL address must start with 'r', got: {}",
            address
        );
        assert!(
            address.len() >= 25 && address.len() <= 34,
            "XRPL address length must be 25-34, got: {}",
            address.len()
        );
    }

    #[test]
    fn test_derive_address_known_vector() {
        // Expected address from xrpl.js:
        // Wallet.fromMnemonic("abandon abandon...", {
        //   derivationPath: "m/44'/144'/0'/0/0",
        //   algorithm: ECDSA.secp256k1
        // }).classicAddress
        let privkey = test_privkey();
        let signer = XrplSigner;
        let address = signer.derive_address(&privkey).unwrap();
        assert_eq!(address, "rHsMGQEkVNJmpGWs8XUBoTBiAAbwxZN5v3");
    }

    #[test]
    fn test_derive_address_deterministic() {
        let privkey = test_privkey();
        let signer = XrplSigner;
        let addr1 = signer.derive_address(&privkey).unwrap();
        let addr2 = signer.derive_address(&privkey).unwrap();
        assert_eq!(addr1, addr2);
    }

    #[test]
    fn test_sign_transaction_single() {
        let privkey = test_privkey();
        let signer = XrplSigner;
        let tx_bytes = b"fake_encoded_tx_bytes_from_encodeForSigning";

        let result = signer.sign_transaction(&privkey, tx_bytes).unwrap();

        // DER signature starts with 0x30
        assert_eq!(result.signature[0], 0x30, "expected DER sequence tag 0x30");
        // secp256k1 DER signatures are 70-72 bytes
        assert!(
            result.signature.len() >= 70 && result.signature.len() <= 72,
            "unexpected DER signature length: {}",
            result.signature.len()
        );
        assert!(result.recovery_id.is_none());
        assert!(result.public_key.is_none());
    }

    #[test]
    fn test_sign_transaction_deterministic() {
        let privkey = test_privkey();
        let signer = XrplSigner;
        let tx_bytes = b"deterministic_test_tx";

        let sig1 = signer.sign_transaction(&privkey, tx_bytes).unwrap();
        let sig2 = signer.sign_transaction(&privkey, tx_bytes).unwrap();
        assert_eq!(
            sig1.signature, sig2.signature,
            "secp256k1 (RFC6979) must be deterministic"
        );
    }

    #[test]
    fn test_sign_transaction_empty_input_errors() {
        let privkey = test_privkey();
        let signer = XrplSigner;
        assert!(signer.sign_transaction(&privkey, b"").is_err());
    }

    #[test]
    fn test_sign_transaction_equals_sign_of_sha512_half() {
        // sign_transaction(privkey, bytes) must equal sign(privkey, sha512_half(bytes))
        // because encodeFor* already contains the prefix — OWS just hashes and signs.
        let privkey = test_privkey();
        let signer = XrplSigner;
        let tx_bytes = b"some_encoded_tx_bytes_with_prefix_already_included";

        let sig_tx = signer.sign_transaction(&privkey, tx_bytes).unwrap();
        let digest = sha512_half(tx_bytes);
        let sig_direct = signer.sign(&privkey, &digest).unwrap();

        assert_eq!(
            sig_tx.signature, sig_direct.signature,
            "sign_transaction must be equivalent to sign(sha512_half(bytes))"
        );
    }

    #[test]
    fn test_sign_raw_32_byte_hash() {
        let privkey = test_privkey();
        let signer = XrplSigner;
        let hash = sha512_half(b"test message");
        let result = signer.sign(&privkey, &hash).unwrap();
        assert_eq!(result.signature[0], 0x30);
    }

    #[test]
    fn test_sign_rejects_non_32_byte_hash() {
        let privkey = test_privkey();
        let signer = XrplSigner;
        assert!(signer.sign(&privkey, b"too short").is_err());
        assert!(signer.sign(&privkey, &[0u8; 33]).is_err());
    }

    #[test]
    fn test_sign_message_unsupported() {
        let privkey = test_privkey();
        let signer = XrplSigner;
        let result = signer.sign_message(&privkey, b"hello");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("not supported"),
            "error should mention 'not supported', got: {}",
            err
        );
    }

    #[test]
    fn test_derive_address_invalid_key() {
        let signer = XrplSigner;
        assert!(signer.derive_address(&[0u8; 16]).is_err());
        assert!(signer.derive_address(&[]).is_err());
    }
}
