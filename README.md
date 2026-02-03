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

## Commands

### Node
```bash
aptly node ledger              # Current ledger info
aptly node health              # Health check
aptly node info                # Node info
aptly node spec                # OpenAPI spec
```

### Account
```bash
aptly account <addr>                          # Account info
aptly account resources <addr>                # All resources
aptly account resource <addr> <type>          # Specific resource
aptly account balance <addr> [asset]          # Balance (default: APT)
aptly account modules <addr>                  # All modules
aptly account module <addr> <name>            # Specific module
aptly account txs <addr> --limit 10           # Transactions
```

### Block
```bash
aptly block <height>                          # By height
aptly block by-version <version>              # By tx version
```

### Transaction
```bash
aptly tx list --limit 10                      # Recent transactions
aptly tx <version_or_hash>                    # View transaction
aptly tx <ver> | aptly tx balance-change      # Balance changes
aptly tx <ver> | aptly tx transfers           # Asset transfers
aptly tx <ver> | aptly tx graph               # Transfer graph
aptly tx trace <version_or_hash>              # Call trace (via Sentio)
cat payload.json | aptly tx simulate <sender> # Simulate
```

### Events
```bash
aptly events <addr> <creation_number> --limit 10
```

### Table
```bash
aptly table item <handle> --key-type <type> --value-type <type> --key <json>
```

### View
```bash
aptly view <function> --type-args <types> --args <json_args>
```

## API Coverage

| Category | Endpoints |
|----------|-----------|
| Node | `GET /`, `/-/healthy`, `/info`, `/spec.json` |
| Account | `GET /accounts/{addr}`, `/resources`, `/resource/{type}`, `/modules`, `/module/{name}`, `/balance/{asset}`, `/transactions` |
| Block | `GET /blocks/by_height/{h}`, `/blocks/by_version/{v}` |
| Transaction | `GET /transactions`, `/by_version/{v}`, `/by_hash/{h}`, `POST /transactions`, `/simulate`, `/encode_submission` |
| Events | `GET /accounts/{addr}/events/{num}` |
| Table | `POST /tables/{handle}/item` |
| View | `POST /view` |

## Design

### Direct HTTP vs SDK

Commands use either direct HTTP calls or the [aptos-go-sdk](https://github.com/aptos-labs/aptos-go-sdk):

- **Direct HTTP**: For commands that print API responses. Output matches the Aptos Node API exactly, ensuring reliable piping to tools like `jq`.
- **SDK**: For commands that process response data (e.g., `aptly tx balance-change` parses transaction events).

This matters for agents: `raw_response != json.Marshal(sdk_struct)` due to field ordering, naming, and serialization differences. Direct HTTP guarantees output fidelity with Aptos API docs.
