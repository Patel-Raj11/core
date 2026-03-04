---
name: lws
description: Lightweight Wallet Signer CLI — generate wallets, derive addresses, and sign messages across EVM, Solana, Bitcoin, Cosmos, and Tron chains.
version: 0.2.0
metadata:
  openclaw:
    requires:
      bins:
        - git
        - cargo
    homepage: https://github.com/dawnlabsai/lws
    os:
      - darwin
      - linux
---

# LWS CLI

Minimal, offline-first CLI for generating wallets, deriving addresses, and signing messages across multiple chains.

## Installation

One-liner:

```bash
curl -fsSL https://openwallet.sh/install.sh | bash
```

The installer will:
1. Install Rust via `rustup` if not already present
2. Clone the repo and build from source
3. Place the `lws` binary at `~/.lws/bin/lws`
4. Add `~/.lws/bin` to your shell's `PATH` (supports zsh, bash, fish)

Set `LWS_INSTALL_DIR` to override the install location.

From source:

```bash
git clone https://github.com/dawnlabsai/lws.git
cd lws/lws
cargo build --workspace --release
cp target/release/lws ~/.lws/bin/lws
```

## Commands

### `lws wallet create`

Create a new wallet — generates a mnemonic and saves a wallet descriptor to the vault.

```
lws wallet create --name <NAME> --chain <CHAIN> [--words 12|24] [--show-mnemonic]
```

- `--name` — Wallet name (required)
- `--chain` — Chain type (required)
- `--words` — Mnemonic word count (default: `12`)
- `--show-mnemonic` — Display the generated mnemonic (for backup only)

### `lws wallet list`

List all saved wallets in the vault.

```
lws wallet list
```

### `lws wallet info`

Show the vault path and list supported chains.

```
lws wallet info
```

### `lws sign message`

Sign a message using a vault wallet with chain-specific formatting (EIP-191 for EVM, etc.).

```
lws sign message --wallet <NAME> --chain <CHAIN> --message <MSG> [--encoding utf8] [--typed-data <JSON>] [--index 0] [--json]
```

- `--wallet` — Wallet name or ID from the vault (required, also via `LWS_WALLET` env)
- `--chain` — Chain type: `evm`, `solana`, `bitcoin`, `cosmos`, `tron` (required)
- `--message` — Message to sign (required)
- `--encoding` — Message encoding: `utf8` or `hex` (default: `utf8`)
- `--typed-data` — EIP-712 typed data JSON (EVM only)
- `--index` — Account index (default: `0`)
- `--json` — Output structured JSON (`signature` + `recovery_id`) instead of raw hex

### `lws sign tx`

Sign a raw transaction using a vault wallet.

```
lws sign tx --wallet <NAME> --chain <CHAIN> --tx <HEX> [--index 0] [--json]
```

- `--wallet` — Wallet name or ID from the vault (required, also via `LWS_WALLET` env)
- `--chain` — Chain type (required)
- `--tx` — Hex-encoded unsigned transaction bytes (required)
- `--index` — Account index (default: `0`)
- `--json` — Output structured JSON (`signature` + `recovery_id`) instead of raw hex

### `lws mnemonic generate`

Generate a new BIP-39 mnemonic phrase.

```
lws mnemonic generate [--words 12|24]
```

- `--words` — Number of mnemonic words, 12 or 24 (default: `12`)

### `lws mnemonic derive`

Derive an address from a mnemonic. The mnemonic is read from the `LWS_MNEMONIC` environment variable or stdin (never passed as a CLI flag).

```
LWS_MNEMONIC="<PHRASE>" lws mnemonic derive --chain <CHAIN> [--index 0]
echo "<PHRASE>" | lws mnemonic derive --chain <CHAIN> [--index 0]
```

- `--chain` — Chain type: `evm`, `solana`, `bitcoin`, `cosmos`, `tron` (required)
- `--index` — Account index (default: `0`)

### `lws update`

Update lws to the latest version by building from the latest commit.

```
lws update [--force]
```

- `--force` — Rebuild even if already on the latest commit

### `lws uninstall`

Remove lws from the system.

```
lws uninstall [--purge]
```

- `--purge` — Also remove all wallet data and config (`~/.lws`)

Removes the binary, cleans PATH entries from shell config files, and optionally deletes the entire `~/.lws` directory. Prompts for confirmation before proceeding.

## File Layout

```
~/.lws/
├── bin/
│   └── lws              # CLI binary
└── wallets/
    └── <wallet-id>.json  # Wallet descriptors
```
