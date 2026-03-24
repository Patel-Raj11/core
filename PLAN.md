# XRPL Chain Support — Implementation Plan

## Scope

Add XRPL as a first-class chain in OWS using **secp256k1 only**. Covers address
derivation and **single-sign only**.
Multisig, batch, and LoanSet signing are out of scope for this phase.
`encode_signed_transaction` is also **out of scope** (same as Bitcoin — `signAndSend`
will return `SignerError::Unsupported` cleanly).

---

## Files to Change

| File | Type | What changes |
|---|---|---|
| `ows/crates/ows-core/src/chain.rs` | Modify | Add `Xrpl` variant throughout |
| `ows/crates/ows-core/src/config.rs` | Modify | Add default RPC for `xrpl:mainnet` |
| `ows/crates/ows-signer/src/chains/xrpl.rs` | **New file** | `XrplSigner` implementation |
| `ows/crates/ows-signer/src/chains/mod.rs` | Modify | Register `XrplSigner` |
| `ows/crates/ows-signer/src/lib.rs` | Modify | Add XRPL to integration tests |

No new Cargo dependencies — `k256`, `sha2`, `ripemd`, `bs58` are already present
in `ows-signer/Cargo.toml`.

---

## Step 1 — `ows-core/src/chain.rs`

Add `Xrpl` to every match arm and array. Exactly mirrors how `Sui` was added.

**`ChainType` enum** — add variant:
```rust
Xrpl,
```

**`ALL_CHAIN_TYPES`** — append (length goes from 8 → 9):
```rust
ChainType::Xrpl,
```

**`KNOWN_CHAINS`** — append entry:
```rust
Chain {
    name: "xrpl",
    chain_type: ChainType::Xrpl,
    chain_id: "xrpl:mainnet",
},
```

**`namespace()`** — add arm:
```rust
ChainType::Xrpl => "xrpl",
```

**`default_coin_type()`** — add arm:
```rust
ChainType::Xrpl => 144,
```

**`from_namespace()`** — add arm:
```rust
"xrpl" => Some(ChainType::Xrpl),
```

**`Display`** — add arm:
```rust
ChainType::Xrpl => "xrpl",
```

**`FromStr`** — add arm:
```rust
"xrpl" => Ok(ChainType::Xrpl),
```

**Tests to update in `chain.rs`:**
- `test_serde_all_variants` — add `(ChainType::Xrpl, "\"xrpl\"")`
- `test_namespace_mapping` — add `assert_eq!(ChainType::Xrpl.namespace(), "xrpl")`
- `test_coin_type_mapping` — add `assert_eq!(ChainType::Xrpl.default_coin_type(), 144)`
- `test_from_namespace` — add `assert_eq!(ChainType::from_namespace("xrpl"), Some(ChainType::Xrpl))`
- `test_all_chain_types` — update length assertion from `8` to `9`

---

## Step 2 — `ows-core/src/config.rs`

Add one entry to `default_rpc()`:
```rust
rpc.insert(
    "xrpl:mainnet".into(),
    "https://xrplcluster.com".into(),
);
```

**Test to update:** `test_load_or_default_nonexistent` — update RPC count from `14` to `15`.

---

## Step 3 — `ows-signer/src/chains/xrpl.rs` (new file)

This is the core implementation. Full file structure:

### 3.1 — Imports and constants

```rust
use crate::curve::Curve;
use crate::traits::{ChainSigner, SignOutput, SignerError};
use k256::ecdsa::{SigningKey, signature::hazmat::PrehashSigner};
use k256::elliptic_curve::sec1::ToEncodedPoint;
use ows_core::ChainType;
use ripemd::Ripemd160;
use sha2::{Digest, Sha256, Sha512};

pub struct XrplSigner;

// Single-sign prefix (HashPrefix.transactionSig in ripple-binary-codec/src/hash-prefixes.ts)
const PREFIX_SINGLE: [u8; 4] = [0x53, 0x54, 0x58, 0x00]; // "STX\0"
```

### 3.2 — `sha512_half` helper

XRPL's hash function: first 32 bytes of SHA-512. Used by all signing modes.

```rust
fn sha512_half(data: &[u8]) -> [u8; 32] {
    let hash = Sha512::digest(data);
    hash[..32].try_into().expect("sha512 output is 64 bytes")
}
```

**xrpl.js parallel:** `sha512Half` in `ripple-binary-codec/src/hashes.ts`

### 3.3 — `sign_secp256k1` helper

Signs a 32-byte digest with secp256k1, returns **DER-encoded** ECDSA signature.
XRPL requires DER (not raw r||s). `k256` provides this via `to_der()`.

```rust
fn sign_secp256k1(private_key: &[u8], digest: &[u8; 32]) -> Result<Vec<u8>, SignerError> {
    let signing_key = SigningKey::from_slice(private_key)
        .map_err(|e| SignerError::InvalidPrivateKey(e.to_string()))?;
    let sig: k256::ecdsa::Signature = signing_key
        .sign_prehash(digest)
        .map_err(|e| SignerError::SigningFailed(e.to_string()))?;
    Ok(sig.to_der().as_bytes().to_vec())
}
```

### 3.4 — `ChainSigner` impl

#### `chain_type`, `curve`, `coin_type`, `default_derivation_path`

```rust
fn chain_type(&self) -> ChainType { ChainType::Xrpl }
fn curve(&self) -> Curve { Curve::Secp256k1 }
fn coin_type(&self) -> u32 { 144 }
fn default_derivation_path(&self, index: u32) -> String {
    format!("m/44'/144'/0'/0/{}", index)
}
```

#### `derive_address`

Algorithm: compressed pubkey → SHA256 → RIPEMD160 → base58check with
XRPL account ID version byte `0x00`.

```rust
fn derive_address(&self, private_key: &[u8]) -> Result<String, SignerError> {
    let signing_key = SigningKey::from_slice(private_key)
        .map_err(|e| SignerError::InvalidPrivateKey(e.to_string()))?;
    let verifying_key = signing_key.verifying_key();
    let pubkey_bytes = verifying_key.to_encoded_point(true); // compressed, 33 bytes

    // SHA256 → RIPEMD160
    let sha256 = Sha256::digest(pubkey_bytes.as_bytes());
    let ripemd = Ripemd160::digest(sha256);

    // Base58Check: version byte 0x00 + 20-byte account_id
    let mut payload = Vec::with_capacity(21);
    payload.push(0x00u8);
    payload.extend_from_slice(&ripemd);

    Ok(bs58::encode(payload)
        .with_alphabet(bs58::Alphabet::RIPPLE)
        .with_check()
        .into_string())
}
```

**xrpl.js parallel:** `deriveAddressFromBytes` = `encodeAccountID(ripemd160(sha256(pubkey)))`
in `ripple-keypairs/src/index.ts:90`

> **Note:** XRPL uses the **Ripple Base58 alphabet**
> (`rpshnaf39wBUDNEGHJKLM4PQRST7VWXYZ2bcdeCg65jkm8oFqi1tuvAxyz`)
> which differs from Bitcoin's. `bs58` crate supports custom alphabets.

#### `sign`

Raw signing over arbitrary pre-hashed bytes. No prefix added here.

```rust
fn sign(&self, private_key: &[u8], message: &[u8]) -> Result<SignOutput, SignerError> {
    let digest: [u8; 32] = message.try_into().map_err(|_| {
        SignerError::InvalidMessage(format!("expected 32-byte hash, got {}", message.len()))
    })?;
    let sig_der = sign_secp256k1(private_key, &digest)?;
    Ok(SignOutput { signature: sig_der, recovery_id: None, public_key: None })
}
```

#### `sign_transaction`

Single-sign only. The caller passes bytes produced by `encodeForSigning(tx)`
from xrpl.js. OWS prepends the `STX\0` prefix, computes SHA512-half, and signs.

```rust
fn sign_transaction(&self, private_key: &[u8], tx_bytes: &[u8]) -> Result<SignOutput, SignerError> {
    if tx_bytes.is_empty() {
        return Err(SignerError::InvalidTransaction("empty input".into()));
    }

    let mut data = Vec::with_capacity(4 + tx_bytes.len());
    data.extend_from_slice(&PREFIX_SINGLE);
    data.extend_from_slice(tx_bytes);

    let digest = sha512_half(&data);
    let sig = sign_secp256k1(private_key, &digest)?;
    Ok(SignOutput { signature: sig, recovery_id: None, public_key: None })
}
```

**xrpl.js parallel:** `signingData(tx, HashPrefix.transactionSig)` → `binary.ts:114`

#### `sign_message`

No XRPL standard exists. For Phase 1 return a clear unsupported error.
This can be upgraded later once a convention is decided.

```rust
fn sign_message(&self, _private_key: &[u8], _message: &[u8]) -> Result<SignOutput, SignerError> {
    Err(SignerError::Unsupported(
        "XRPL has no canonical off-chain message signing standard. \
         Implement a convention (e.g. SHA512Half(XMSG\\0 || message)) \
         before enabling this.".into(),
    ))
}
```

#### `encode_signed_transaction`

Out of scope. Return unsupported — same pattern as Bitcoin.

```rust
fn encode_signed_transaction(&self, _tx_bytes: &[u8], _sig: &SignOutput) -> Result<Vec<u8>, SignerError> {
    Err(SignerError::Unsupported(
        "XRPL encode_signed_transaction not implemented (Phase 2). \
         Use xrpl.js to assemble the signed transaction.".into(),
    ))
}
```

### 3.5 — Unit tests in `xrpl.rs`

Every test uses the known `"abandon abandon ... about"` mnemonic with
derivation path `m/44'/144'/0'/0/0` and verifies against xrpl.js output.

| Test | What it checks |
|---|---|
| `test_chain_properties` | `chain_type`, `curve`, `coin_type` return correct values |
| `test_derivation_path` | `default_derivation_path(0)` = `m/44'/144'/0'/0/0`, index increments correctly |
| `test_derive_address_format` | Address starts with `r`, length 25–34 |
| `test_derive_address_known_vector` | Address matches xrpl.js `Wallet.fromMnemonic("abandon...", {derivationPath: "m/44'/144'/0'/0/0"}).classicAddress` |
| `test_sign_single` | `SHA512Half(STX\0 \|\| tx)` output verifiable with `ripple-keypairs.verify()` |
| `test_sign_empty_input` | Returns error |
| `test_der_signature_format` | Signature bytes parse as valid DER (starts with `0x30`) |
| `test_sign_message_unsupported` | Returns `SignerError::Unsupported` |
| `test_encode_signed_transaction_unsupported` | Returns `SignerError::Unsupported` |
| `test_deterministic_signing` | Same input → same DER signature (k256 deterministic RFC6979) |

---

## Step 4 — `ows-signer/src/chains/mod.rs`

Two additions:

```rust
pub mod xrpl;
pub use self::xrpl::XrplSigner;
```

```rust
ChainType::Xrpl => Box::new(XrplSigner),
```

---

## Step 5 — `ows-signer/src/lib.rs` (integration tests)

### 5.1 — Add to `test_full_pipeline_*`

Add a new test:
```rust
#[test]
fn test_full_pipeline_xrpl() {
    let mnemonic = Mnemonic::from_phrase(ABANDON_PHRASE).unwrap();
    let address = derive_address_for_chain(&mnemonic, ChainType::Xrpl);
    assert!(address.starts_with('r'), "XRPL address must start with r, got: {}", address);
    assert!(address.len() >= 25 && address.len() <= 34,
        "XRPL address length out of range: {}", address.len());
}
```

### 5.2 — Add to `test_cross_chain_different_addresses`

```rust
let xrpl_addr = derive_address_for_chain(&mnemonic, ChainType::Xrpl);
// add &xrpl_addr to the addrs array
```

### 5.3 — Add to `test_signer_for_chain_registry`

```rust
ChainType::Xrpl,  // add to the chain list
```

---


## After Every Step — Quality Gates

Run these from the `ows/` directory after completing each step before moving on.

**Tests:**
```bash
cd ows && cargo test --workspace
```

**Formatting:**
```bash
cd ows && cargo fmt --all
```

**Linting (must be warning-free):**
```bash
cd ows && cargo clippy --workspace -- -D warnings
```

All three must pass cleanly before proceeding to the next step.

---

## Final Verification

After all steps are complete, run the full suite one last time:

```bash
cd ows && cargo test --workspace
cd ows && cargo fmt --all
cd ows && cargo clippy --workspace -- -D warnings
```

Cross-check the derived address from `test_full_pipeline_xrpl` against:

```js
// xrpl.js verification
const { Wallet, ECDSA } = require('xrpl')
const w = Wallet.fromMnemonic(
  "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
  { derivationPath: "m/44'/144'/0'/0/0", algorithm: ECDSA.secp256k1 }
)
console.log(w.classicAddress)  // must match OWS output
```

Cross-check a single-sign signature against:

```js
// ripple-keypairs verification
const keypairs = require('ripple-keypairs')
keypairs.verify(messageHex, signatureHex, publicKeyHex)  // must return true
```
