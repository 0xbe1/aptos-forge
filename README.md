# aptly

The best way to interact with Aptos blockchain from your terminal. Built for both humans and AI agents — every command returns structured, parseable output that works seamlessly with LLMs and automation pipelines.

## Installation

```bash
# Install latest release binary (macOS/Linux)
curl -sSL https://raw.githubusercontent.com/0xbe1/aptly/main/install.sh | sh
```

From source:

```bash
cargo install --path crates/aptly-cli --bin aptly
```

## Claude Code Integration

```bash
# 1) Install aptly
curl -sSL https://raw.githubusercontent.com/0xbe1/aptly/main/install.sh | sh

# 2) Install Claude Code skill
curl -sSL https://raw.githubusercontent.com/0xbe1/aptly/main/install-skill.sh | sh
```

## Usage

```bash
# Use mainnet (default)
aptly node ledger

# Use custom RPC
aptly --rpc-url https://rpc.sentio.xyz/aptos/v1 node ledger
```

## Value-add Commands

```bash
# Address labels
aptly address thala

# Account source code (if published with metadata)
aptly account source-code <address> [module_name] [--package <name>] [--raw]

# If source metadata is unavailable, decompile bytecode instead
aptly decompile address <address>
aptly decompile module <address> <module_name>

# Outgoing sends from an account
aptly account sends <address> --limit 25 [--pretty]

# Balance changes from tx (supports stdin piping)
aptly tx balance-change <version_or_hash> [--aggregate]
aptly tx <version_or_hash> | aptly tx balance-change --aggregate
```

## Decompile Plugin (`move-decompiler`)

`aptly` integrates with Aptos `move-decompiler` as an optional plugin.
Use decompile as the default fallback whenever `aptly account source-code` cannot return source.

```bash
# Check plugin installation
aptly plugin list
aptly plugin doctor

# Decompile all modules under an address
aptly decompile address 0x1

# Decompile one module
aptly decompile module 0x1 coin

# Raw passthrough to move-decompiler
aptly decompile raw -- --help
```

If `move-decompiler` is not on `PATH`:

```bash
export APTLY_MOVE_DECOMPILER_BIN=/path/to/move-decompiler
```

Binary resolution order for `aptly decompile` is:
1. `--decompiler-bin /path/to/move-decompiler` (if provided)
2. `APTLY_MOVE_DECOMPILER_BIN`
3. `PATH` (`move-decompiler`)

Default wrapper output: `decompiled/<address>/`

## Script Compose Plugin (`aptos-script-compose`)

`aptly` can discover `aptos-script-compose` as an external plugin binary.
The binary wraps Aptos `script-composer` and compiles batched call payload JSON from stdin.

```bash
# Check plugin installation
aptly plugin list
aptly plugin doctor --script-compose-bin /path/to/aptos-script-compose

# Compose batched script payload from stdin and print raw 0x-hex serialized Script bytes
cat compose_payload.json | aptos-script-compose --rpc-url https://rpc.sentio.xyz/aptos/v1

# Emit a JSON payload directly consumable by aptly tx simulate / submit
cat compose_payload.json \
  | aptos-script-compose --rpc-url https://rpc.sentio.xyz/aptos/v1 --emit-script-payload \
  | aptly tx simulate <sender>
```

If `aptos-script-compose` is not on `PATH`:

```bash
export APTLY_APTOS_SCRIPT_COMPOSE_BIN=/path/to/aptos-script-compose
```

Binary resolution order for `aptos-script-compose` plugin checks:
1. `--script-compose-bin /path/to/aptos-script-compose` (for `aptly plugin doctor`)
2. `APTLY_APTOS_SCRIPT_COMPOSE_BIN`
3. `PATH` (`aptos-script-compose`)

## Trace API (Sentio)

`aptly tx trace` queries Sentio's hosted trace API at `https://app.sentio.xyz`.
Network ID is auto-detected from the selected `--rpc-url` (for example, Aptos mainnet `1`
or Movement mainnet `126`).
Default hosted tracing is usually much faster. Try local mode only when your RPC is very fast
(for example, a self-hosted node).

```bash
# Option 1: Sentio hosted API (default)
aptly tx trace <tx_version_or_hash>

# Option 2: local aptos-tracer binary
aptly tx trace <tx_version_or_hash> --local-tracer

# Optional: pin a specific local binary path
aptly tx trace <tx_version_or_hash> --local-tracer /path/to/aptos-tracer
```

For `--local-tracer`, binary resolution order is:
1. `--local-tracer /path/to/aptos-tracer` (if provided)
2. `APTLY_APTOS_TRACER_BIN`
3. `PATH` (`aptos-tracer`)

## Transaction Helpers

```bash
# Basic transaction fetch/list
aptly tx <version_or_hash>
aptly tx list --limit 10

# Encode unsigned transaction for external signing
cat unsigned_tx.json | aptly tx encode

# Simulate entry function payload from stdin (no private key required)
cat payload.json | aptly tx simulate <sender>

# Trace transaction call tree (Sentio API)
aptly tx trace <version_or_hash>

# Submit signed transaction JSON
cat signed_tx.json | aptly tx submit
```

## Thin wrappers over Aptos Node API

```bash
# Node
aptly node ledger|health|info|spec|estimate-gas-price

# Account
aptly account <addr>
aptly account resources|modules <addr>
aptly account resource <addr> <type>
aptly account module <addr> <name>
aptly account balance <addr> [asset]
aptly account txs <addr> --limit 10

# Block
aptly block <height>
aptly block by-version <version>

# Events
aptly events <addr> <creation_number> --limit 10

# Table
aptly table item <handle> --key-type <type> --value-type <type> --key <json>

# View
aptly view <function> --type-args <types> --args <json_args>
```
