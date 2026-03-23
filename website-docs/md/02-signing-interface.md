# Signing Interface

> The core operations exposed by an OWS implementation: signing, sending, and message signing.

## Interface Definition

### `sign(request: SignRequest): Promise<SignResult>`

Signs a transaction without broadcasting it. Returns the signed transaction bytes.

```typescript
interface SignRequest {
  walletId: WalletId;
  chainId: ChainId;       // CAIP-2 or supported shorthand alias
  transactionHex: string; // hex-encoded serialized transaction bytes
}

interface SignResult {
  signature: string;
  recoveryId?: number;
}
```

**Flow:**
1. Resolve `walletId` → wallet file
2. Resolve `chainId` → chain plugin
3. Authenticate caller: owner (passphrase/passkey) or agent (API key)
4. If agent: verify wallet is in API key's `walletIds` scope; evaluate API key's policies against the transaction
5. If owner: skip policy evaluation (sudo access)
6. If policies pass (or owner), decrypt key material
7. Sign via chain plugin's signer
8. Wipe key material
9. Return the signature (and recovery ID when applicable)

### `signAndSend(request: SignAndSendRequest): Promise<SignAndSendResult>`

Signs, encodes, and broadcasts a transaction.

```typescript
interface SignAndSendRequest extends SignRequest {
  rpcUrl?: string;
}

interface SignAndSendResult {
  transactionHash: string;
}
```

The signer implementation handles transaction encoding and broadcasting via the resolved RPC endpoint. Current implementations do not wait for confirmations or return a richer status object.

#### CLI: `ows sign send-tx`

The `ows sign send-tx` command provides sign-and-broadcast from the command line:

```bash
ows sign send-tx \
  --chain evm \
  --wallet agent-treasury \
  --tx 0x<hex-encoded-unsigned-tx> \
  --index 0 \
  --rpc-url https://eth-sepolia.g.alchemy.com/v2/demo   # optional override
```

The command signs the transaction using the wallet's encrypted secret, resolves the RPC endpoint (flag > config override > built-in default), broadcasts via the chain-appropriate protocol, and prints the transaction hash. Use `--json` for structured output including `tx_hash` and `chain`.

Per-chain broadcast protocols:

| Chain | Broadcast Method |
|---|---|
| EVM | JSON-RPC `eth_sendRawTransaction` |
| Solana | JSON-RPC `sendTransaction` (base64-encoded) |
| Bitcoin | POST raw hex to `{rpc}/tx` (mempool.space REST) |
| Cosmos | POST to `{rpc}/cosmos/tx/v1beta1/txs` (base64 tx_bytes) |
| Tron | POST to `{rpc}/wallet/broadcasthex` |
| TON | POST to `{rpc}/sendBoc` |
| Sui | JSON-RPC `sui_executeTransactionBlock` (base64 tx + sig) |

### `signMessage(request: SignMessageRequest): Promise<SignMessageResult>`

Signs an arbitrary message (for authentication, attestation, or off-chain signatures like EIP-712).

```typescript
interface SignMessageRequest {
  walletId: WalletId;
  chainId: ChainId;
  message: string | Uint8Array;
  encoding?: "utf8" | "hex";
  typedData?: TypedData;               // EIP-712 typed data (EVM only)
}

interface SignMessageResult {
  signature: string;
  recoveryId?: number;                 // for secp256k1 recovery
}
```

Message signing follows chain-specific conventions:
- **EVM**: `personal_sign` (EIP-191) or `eth_signTypedData_v4` (EIP-712)
- **Solana**: Ed25519 signature over the raw message bytes
- **Sui**: Intent-prefixed (scope=3) BLAKE2b-256 digest, Ed25519 signature
- **Cosmos**: ADR-036 off-chain signing
- **Filecoin**: Blake2b-256 hash then secp256k1 signing

### `signTypedData(request: SignTypedDataRequest): Promise<SignMessageResult>`

Signs EIP-712 typed structured data. This is a dedicated operation separate from `signMessage` to provide a clean SDK interface for typed data signing without overloading the message signing API.

```typescript
interface SignTypedDataRequest {
  walletId: WalletId;
  chainId: ChainId;                    // Must be an EVM chain
  typedDataJson: string;               // JSON string of EIP-712 typed data
}
```

The `typedDataJson` field must be a JSON string containing the standard EIP-712 fields: `types`, `primaryType`, `domain`, and `message`.

```json
{
  "types": {
    "EIP712Domain": [
      {"name": "name", "type": "string"},
      {"name": "chainId", "type": "uint256"}
    ],
    "Transfer": [
      {"name": "to", "type": "address"},
      {"name": "amount", "type": "uint256"}
    ]
  },
  "primaryType": "Transfer",
  "domain": {"name": "MyDApp", "chainId": "1"},
  "message": {"to": "0xabc...", "amount": "1000"}
}
```

Returns a `SignMessageResult` with the signature and recovery ID. Only supported for EVM chains.

## Serialized Transaction Format

Current OWS implementations accept **already-serialized transaction bytes encoded as hex**. OWS signs those bytes, and for broadcast-capable chains it encodes the signed transaction into the wire format expected by the chain RPC.

## Error Handling

| Code | Meaning |
|---|---|
| `WALLET_NOT_FOUND` | No wallet with the given ID exists |
| `CHAIN_NOT_SUPPORTED` | No signer is available for the given chain |
| `INVALID_PASSPHRASE` | Vault passphrase was incorrect |
| `INVALID_INPUT` | Request payload or arguments were malformed |
| `CAIP_PARSE_ERROR` | The chain identifier could not be parsed |
| `POLICY_DENIED` | Request was rejected by the policy engine |
| `API_KEY_NOT_FOUND` | The provided API token did not resolve to a key |
| `API_KEY_EXPIRED` | The API key has expired |

## Concurrency

Current implementations do not provide a per-wallet nonce manager or explicit same-wallet request serialization. Callers that need strict nonce coordination must currently handle it at a higher level.

## References

- [EIP-191: Signed Data Standard](https://eips.ethereum.org/EIPS/eip-191)
- [EIP-712: Typed Structured Data](https://eips.ethereum.org/EIPS/eip-712)
