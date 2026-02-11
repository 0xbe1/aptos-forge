# aptly

Best Aptos CLI for agents.

## Installation

```bash
curl -sSL https://raw.githubusercontent.com/0xbe1/aptly/main/install.sh | sh
```

## Usage

```bash
# Use mainnet (default)
aptly node ledger

# Use custom RPC
aptly --rpc-url https://rpc.sentio.xyz/aptos/v1 node ledger
```

## Highlighted Commands

```bash
# Resolve known protocol labels to on-chain addresses
$ aptly address thala
{
  "0x007730cd28ee1cdc9e999336cbc430f99e7c44397c0aa77516f6f23a78559bb5": "ThalaSwap v2",
  "0x075b4890de3e312d9425408c43d9a9752b64ab3562a30e89a55bdc568c645920": "ThalaSwap CL",
  "0x48271d39d0b05bd6efca2278f22277d6fcc375504f9839fd73f74ace240861af": "ThalaSwap v1",
  "0x60955b957956d79bc80b096d3e41bad525dd400d8ce957cdeb05719ed1e4fc26": "Thala router",
  "0x6b3720cd988adeaf721ed9d4730da4324d52364871a68eac62b46d21e4d2fa99": "Thala Farm",
  "0x6f986d146e4a90b828d8c12c14b6f4e003fdff11a8eecceceb63744363eaac01": "Thala CDP",
  "0xcb8365dc9f7ac6283169598aaad7db9c7b12f52da127007f37fa4565170ff59c": "ThalaSwap CL Farm",
  "0xfaf4e633ae9eb31366c9ca24214231760926576c7b625313b3688b5e900731f6": "Thala LSD"
}

# Read published source metadata when available
$ aptly account source-code 0x1 chain_id --raw | head -n 20
/// The chain id distinguishes between different chains (e.g., testnet and the main network).
/// One important role is to prevent transactions intended for one chain from being executed on another.
/// This code provides a container for storing a chain id and functions to initialize and get it.
module aptos_framework::chain_id {
    use aptos_framework::system_addresses;

    friend aptos_framework::genesis;

    struct ChainId has key {
        id: u8
    }

    /// Only called during genesis.
    /// Publish the chain ID `id` of this instance under the SystemAddresses address
    public(friend) fun initialize(aptos_framework: &signer, id: u8) {
        system_addresses::assert_aptos_framework(aptos_framework);
        move_to(aptos_framework, ChainId { id })
    }

    #[view]

# If source metadata is missing, decompile module bytecode instead
$ aptly decompile address 0x8b4a2c4bb53857c718a04c020b98f8c2e1f99a68b0f57389a8bf5434cd22e05c --module pool_v3 && grep "fun add_liquiditiy" decompiled/0x8b4a2c4bb53857c718a04c020b98f8c2e1f99a68b0f57389a8bf5434cd22e05c/pool_v3.move
public fun add_liquidity(p0: &signer, p1: object::Object<position_v3::Info>, p2: u128, p3: fungible_asset::FungibleAsset, p4: fungible_asset::FungibleAsset): (u64, u64, fungible_asset::FungibleAsset, fungible_asset::FungibleAsset) {
friend fun add_liquidity_v2(p0: &signer, p1: object::Object<position_v3::Info>, p2: u128, p3: fungible_asset::FungibleAsset, p4: fungible_asset::FungibleAsset): (u64, u64, fungible_asset::FungibleAsset, fungible_asset::FungibleAsset)

# Summarize asset deltas in a transaction
$ aptly tx balance-change 4300326632 --aggregate
[
  {
    "account": "0x623d5561daf5a4cdcd234b0f9343016c53012236fe2e0926e1d2f7251191c33",
    "amount": "1034475000",
    "asset": "0xa"
  },
  {
    "account": "0x623d5561daf5a4cdcd234b0f9343016c53012236fe2e0926e1d2f7251191c33",
    "amount": "-9960666",
    "asset": "0xbae207659db88bea0cbead6da0ed00aac12edcdda169e591cd41c94180b46f3b"
  },
  {
    "account": "0x75b4890de3e312d9425408c43d9a9752b64ab3562a30e89a55bdc568c645920",
    "amount": "2490",
    "asset": "0xbae207659db88bea0cbead6da0ed00aac12edcdda169e591cd41c94180b46f3b"
  },
  {
    "account": "0xa8a355df7d9e75ef16082da2a0bad62c173a054ab1e8eae0f0e26c828adaa4ef",
    "amount": "9958176",
    "asset": "0xbae207659db88bea0cbead6da0ed00aac12edcdda169e591cd41c94180b46f3b"
  },
  {
    "account": "0xa8a355df7d9e75ef16082da2a0bad62c173a054ab1e8eae0f0e26c828adaa4ef",
    "amount": "-1034482800",
    "asset": "0xa"
  }
]

# Inspect the transaction call tree
$ aptly tx trace 0xf44b2ea4a0cd55a31559fc022a2fba12aa81c46dcfce31a050d9d42d93a7dae5 | jq -r '
    def show($d):
      (("  " * $d) + (.contractName + "::" + .functionName)),
      (if $d < 2 then .calls[]? | show($d+1) else empty end);
    show(0)
  '
primary_fungible_store::primary_fungible_store::transfer
  transaction_arg_validation::transaction_arg_validation::validate_combine_signer_and_txn_args
    object::object::address_to_object
  primary_fungible_store::object::object_address
  primary_fungible_store::object::create_user_derived_object_address
  primary_fungible_store::fungible_asset::store_exists
  primary_fungible_store::object::address_to_object
  primary_fungible_store::object::is_burnt
  primary_fungible_store::object::object_address
  primary_fungible_store::object::create_user_derived_object_address
  primary_fungible_store::fungible_asset::store_exists
  primary_fungible_store::object::address_to_object
  primary_fungible_store::dispatchable_fungible_asset::withdraw
    dispatchable_fungible_asset::fungible_asset::withdraw_sanity_check
    dispatchable_fungible_asset::fungible_asset::withdraw_permission_check
    dispatchable_fungible_asset::fungible_asset::withdraw_dispatch_function
    dispatchable_fungible_asset::option::is_some
    dispatchable_fungible_asset::option::borrow
    dispatchable_fungible_asset::function_info::load_module_from_function
    dispatchable_fungible_asset::fungible_asset::store_metadata
    dispatchable_fungible_asset::object::object_address
    dispatchable_fungible_asset::usdt::withdraw
  primary_fungible_store::dispatchable_fungible_asset::deposit
    dispatchable_fungible_asset::fungible_asset::deposit_sanity_check
    dispatchable_fungible_asset::fungible_asset::deposit_dispatch_function
    dispatchable_fungible_asset::option::is_some
    dispatchable_fungible_asset::option::borrow
    dispatchable_fungible_asset::function_info::load_module_from_function
    dispatchable_fungible_asset::fungible_asset::store_metadata
    dispatchable_fungible_asset::object::object_address
    dispatchable_fungible_asset::usdt::deposit
```

## Other Commands

Thin wrappers over Aptos Node API.

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

# Tx
aptly tx <version_or_hash>
aptly tx list --limit 25 --start 0
aptly tx encode < unsigned_txn.json
aptly tx simulate <sender_address> < payload.json
aptly tx submit < signed_txn.json
aptly tx trace <version_or_hash> [--local-tracer [tracer_bin]]
aptly tx balance-change [version_or_hash] [--aggregate]
```

## TODOs

- [ ] install script for aptos-script-compose
- [ ] aptos-script-compose should skip the top layer "steps"
- [ ] add tx compose subcommand
- [ ] decompile to stdout
- [ ] decompile args should be identical to source-code args
- [ ] visualize tx trace with --open
