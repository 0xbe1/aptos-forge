---
name: aptos
description: Explore Aptos blockchain using aptly CLI. Use when asked about Aptos contracts, modules, transactions, accounts, balances, or protocol interfaces like "show me interface of X".
---

# Aptos Blockchain Explorer

Use the `aptly` CLI to explore the Aptos blockchain. Requires aptly to be installed.

## Key Commands

### Find protocol addresses
`aptly address <name>` - Search ThalaLabs address labels

### Explore accounts
- `aptly account modules <addr>` - List all modules
- `aptly account module <addr> <name>` - Get module ABI
- `aptly account resources <addr>` - List all resources
- `aptly account balance <addr>` - Check balance

### Analyze transactions
- `aptly tx <version_or_hash>` - Get transaction
- `aptly tx <ver> | aptly tx balance-change` - Balance changes
- `aptly tx <ver> | aptly tx graph --pretty` - Transfer flow
- `aptly tx trace <ver>` - Call trace (via Sentio)

### Other
- `aptly node ledger` - Current ledger info
- `aptly view <function> --args <json>` - Call view function

## Interpreting Module ABIs

From `aptly account module` output:
- `exposed_functions` with `is_entry: true` → transaction entry points
- `exposed_functions` with `is_view: true` → read-only view functions
- `structs` → data types with fields and Move abilities

Present as clean Move function signatures.
