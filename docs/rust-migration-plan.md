# aptly Rust Migration Plan

## Why migrate

- Aptos is Rust-first, so moving `aptly` to Rust reduces language/context switching and makes deeper protocol integrations easier.
- `aptos-rust-sdk` can replace most direct `aptos-go-sdk` usage.
- The current product is mostly a CLI wrapper over Aptos Node API, with a small set of value-add analytics commands.

## Scope

### In scope (v1 parity + near-term roadmap)

- Preserve existing CLI shape and JSON output expectations where practical.
- Migrate current commands to Rust.
- Re-implement core value-add features:
  - `account source-code`
  - `account sends`
  - `address`
  - `tx balance-change`
- Keep `tx trace` via Sentio initially behind a provider abstraction.
- Add plugin architecture for optional `decompile` command, backed by Aptos
  `move-decompiler`:
  `https://github.com/aptos-labs/aptos-core/blob/main/third_party/move/tools/move-decompiler`.

### Out of scope (first cut)

- Building a proprietary trace engine immediately (we keep Sentio provider first).
- Redesigning command UX/output format unless needed for correctness.
- Breaking changes to install/release UX (`install.sh`, binary name `aptly`).

## Current command surface (to preserve)

- Thin API wrappers: `node`, `account`, `block`, `events`, `table`, `view`, basic `tx` fetch/list/submit/simulate.
- Value-add commands: `account source-code`, `account sends`, `address`, `tx balance-change`, `tx graph`, `tx trace`.

## Target Rust architecture

Use a Rust workspace to separate concerns and keep optional features modular.

```text
aptly/
  Cargo.toml (workspace)
  crates/
    aptly-cli/              # clap command definitions and output formatting
    aptly-core/             # shared domain types, errors, helpers
    aptly-aptos/            # aptos-rust-sdk + node API adapter layer
    aptly-analytics/        # sends / balance-change / graph logic
    aptly-trace/            # TraceProvider trait + models
    aptly-trace-sentio/     # Sentio implementation (default provider)
    aptly-plugin/           # plugin discovery/execution utilities
```

### Design principles

- Keep business logic out of CLI parsing code.
- Encapsulate SDK vs raw REST behind traits so replacement is cheap.
- Snapshot-test JSON outputs for parity.
- Prefer additive feature flags/plugins over hard dependencies.

## Command migration strategy

1. **Parity-first wrappers**: move all plain REST wrapper commands quickly.
2. **Value-add ports second**: port custom logic with golden test fixtures.
3. **Plugin hooks third**: add extension points for decompile/trace backends.

### Feature-by-feature migration notes

#### `account source-code`

- Query `0x1::code::PackageRegistry` resource.
- Parse `packages[*].modules[*].source`, then hex decode + gzip inflate.
- Keep current filters (`--package`, optional module arg, `--raw`).
- Preserve current error semantics (missing module, no metadata, etc.).

#### `account sends`

- Parse account transactions and detect entry functions:
  - `0x1::aptos_account::transfer_coins`
  - `0x1::primary_fungible_store::transfer`
  - `0x1::coin::transfer`
- Keep metadata lookup cache for symbol/decimals.
- Preserve `--limit` and `--pretty` behavior.

#### `address`

- Continue fetching labels from ThalaLabs labels JSON.
- Add a small local cache with TTL (optional) to reduce repeated network fetches.

#### `tx balance-change`

- Port event + write-set interpretation exactly:
  - gas fee as sender debit in APT
  - `fungible_asset::Withdraw/Deposit` events
  - transfer store owner/asset resolution from tx changes with fallback resource reads
- Preserve `--aggregate` contract.

#### `tx trace`

- Create `TraceProvider` trait in `aptly-trace`.
- Implement Sentio provider in `aptly-trace-sentio` (default in v1).
- Future provider: in-house Rust trace library implementing same trait.

## Plugin plan (`decompile`)

### Goal

`decompile` is optional and should only work when user explicitly installs plugin dependencies.
The backend decompiler is Aptos `move-decompiler` from:
`https://github.com/aptos-labs/aptos-core/blob/main/third_party/move/tools/move-decompiler`.

### Approach

- Add plugin command group:
  - `aptly plugin list`
  - `aptly plugin doctor`
- Add optional command:
  - `aptly decompile ...`
- Runtime behavior:
  - detect required dependency (binary/lib path/config)
  - fail with actionable install message if missing
  - pin/record tested `aptos-core` commit or release for reproducibility
- Keep decompiler integration isolated so base CLI remains lightweight.

## Phased execution plan

## Phase 0: Specification and freeze (1 week)

- Define command parity matrix (Go vs Rust).
- Freeze output contracts for commands relied on by agents/pipes.
- Record fixture inputs/expected JSON for value-add commands.

## Phase 1: Workspace bootstrap + plain wrappers (1-2 weeks)

- Scaffold Rust workspace and CI.
- Implement global flags (`--rpc-url`) and root command tree.
- Port thin wrapper commands first.
- Ship `aptly-rs` preview binary for internal use.

## Phase 2: Value-add command ports (2 weeks)

- Port `account source-code`, `account sends`, `address`, `tx balance-change`.
- Port `tx graph` (shares balance-change/store metadata helpers).
- Add fixture-driven integration tests + snapshot outputs.

## Phase 3: Trace provider abstraction (1 week)

- Implement `TraceProvider` + Sentio provider.
- Keep command UX compatible with current `tx trace`.
- Add config point for provider selection (default `sentio`).

## Phase 4: Decompile plugin (1-2 weeks)

- Add plugin discovery and dependency checks.
- Implement first `decompile` adapter to Aptos `move-decompiler`
  (`aptos-core/third_party/move/tools/move-decompiler`).
- Document install steps and failure diagnostics.

## Phase 5: Cutover + deprecation (1 week)

- Publish Rust binary as `aptly`.
- Keep Go implementation on maintenance branch for rollback window.
- Announce compatibility notes and migration guidance.

## CI/CD and packaging changes

- Replace Go build pipeline with Cargo-based matrix builds for:
  - `darwin/amd64`
  - `darwin/arm64`
  - `linux/amd64`
  - `linux/arm64`
- Keep artifact names and install script contract stable so users do not need to change commands.
- Embed version metadata via build-time env vars (equivalent to current ldflags metadata).

## Quality gates

- **Golden snapshots** for key JSON outputs.
- **Fixture tests** using known tx versions/hashes for analytics commands.
- **Compatibility checks** comparing Go and Rust outputs for the same inputs.
- **Smoke tests** for critical commands on mainnet RPC.

## Risks and mitigations

- SDK API mismatch vs Go behavior
  - Mitigation: keep a low-level REST fallback adapter in `aptly-aptos`.
- Output drift breaks agent workflows
  - Mitigation: snapshot tests + explicit compatibility matrix.
- Optional plugin dependency complexity
  - Mitigation: plugin doctor checks and clear install docs.
- External trace API instability
  - Mitigation: provider abstraction + retries/timeouts + future in-house provider.

## Deliverables checklist

- [ ] Rust workspace with command parity baseline
- [ ] Ported value-add commands (`source-code`, `sends`, `address`, `balance-change`)
- [ ] Trace provider abstraction + Sentio provider
- [ ] Optional decompile plugin integration
- [ ] Updated release workflow and install path
- [ ] Migration guide (`Go -> Rust`) and compatibility notes

## Suggested rollout decision points

1. **After Phase 1**: verify wrapper parity and shell/script compatibility.
2. **After Phase 2**: verify value-add output parity against Go fixtures.
3. **After Phase 4**: validate plugin UX before publicizing `decompile`.
4. **Before cutover**: run both CLIs in parallel for at least one release cycle.
