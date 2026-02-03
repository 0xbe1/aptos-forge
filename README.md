# aptly

Aptos CLI for agents.

## Installation

```bash
go install github.com/0xbe1/aptly@latest
```

## Usage

```bash
# Use mainnet (default)
aptly node ledger

# Use custom RPC
aptly --rpc-url https://rpc.sentio.xyz/aptos/v1 node ledger
```

## The Fun Parts

Analyze transactions with derived insights:

```bash
aptly tx <ver> | aptly tx balance-change      # Balance changes per account
aptly tx <ver> | aptly tx transfers           # Withdraw/Deposit events
aptly tx <ver> | aptly tx graph               # Transfer flow (from → to)
aptly tx <ver> | aptly tx graph --pretty      # Human-readable with symbols
aptly tx trace <version_or_hash>              # Call trace (via Sentio)
cat payload.json | aptly tx simulate <sender> # Simulate, then pipe to above
```

## The Boring Parts

Thin wrappers over [Aptos Node API](https://aptos.dev/en/build/apis/fullnode-rest-api):

```bash
# Node
aptly node ledger|health|info|spec

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

# Transaction
aptly tx list --limit 10
aptly tx <version_or_hash>

# Events
aptly events <addr> <creation_number> --limit 10

# Table
aptly table item <handle> --key-type <type> --value-type <type> --key <json>

# View
aptly view <function> --type-args <types> --args <json_args>
```
