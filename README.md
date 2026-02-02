# apt

Aptos CLI for agents.

## Installation

```bash
go install github.com/0xbe1/apt@latest
```

## Usage

```bash
# Use mainnet (default)
apt node ledger

# Use custom RPC
apt --rpc-url https://rpc.sentio.xyz/aptos/v1 node ledger
```

## Commands

### Node
```bash
apt node ledger              # Current ledger info
apt node health              # Health check
apt node info                # Node info
apt node spec                # OpenAPI spec
```

### Account
```bash
apt account <addr>                          # Account info
apt account resources <addr>                # All resources
apt account resource <addr> <type>          # Specific resource
apt account balance <addr> [asset]          # Balance (default: APT)
apt account modules <addr>                  # All modules
apt account module <addr> <name>            # Specific module
apt account txs <addr> --limit 10           # Transactions
```

### Block
```bash
apt block <height>                          # By height
apt block by-version <version>              # By tx version
```

### Transaction
```bash
apt tx list --limit 10                      # Recent transactions
apt tx <version_or_hash>                    # View transaction
apt tx <ver> | apt tx balance-change        # Balance changes
apt tx <ver> | apt tx transfers             # Asset transfers
apt tx <ver> | apt tx graph                 # Transfer graph
apt tx trace <version_or_hash>              # Call trace (via Sentio)
cat payload.json | apt tx simulate <sender> # Simulate
```

### Events
```bash
apt events <addr> <creation_number> --limit 10
```

### Table
```bash
apt table item <handle> --key-type <type> --value-type <type> --key <json>
```

### View
```bash
apt view <function> --type-args <types> --args <json_args>
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
