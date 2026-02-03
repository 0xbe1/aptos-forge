# Aptly

Aptos blockchain CLI. Use `aptly` commands for any Aptos exploration tasks. See README.md for commands.

## ABI Interpretation

When showing a contract interface from `aptly account module` output:

- `exposed_functions` with `is_entry: true` → transaction entry points
- `exposed_functions` with `is_view: true` → read-only view functions
- `structs` → data types with fields and Move abilities

Present as clean Move function signatures.
