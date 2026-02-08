package tx

import (
	"encoding/json"
	"fmt"
	"math/big"
	"os"
	"strings"

	"github.com/0xbe1/aptly/pkg/api"
	"github.com/aptos-labs/aptos-go-sdk"
	aptosapi "github.com/aptos-labs/aptos-go-sdk/api"
	"github.com/spf13/cobra"
)

var aggregateFlag bool

var balanceChangeCmd = &cobra.Command{
	Use:   "balance-change [version_or_hash]",
	Short: "Show balance changes in a transaction",
	Long:  `Lists gas fee, withdraw, and deposit events. Use --aggregate for net balance changes per account.`,
	Args:  cobra.MaximumNArgs(1),
	RunE:  runBalanceChange,
}

func init() {
	balanceChangeCmd.Flags().BoolVar(&aggregateFlag, "aggregate", false, "Show net balance change per account per asset")
}

type BalanceChange struct {
	Type          string `json:"type"`
	Account       string `json:"account"`
	FungibleStore string `json:"fungible_store"`
	Asset         string `json:"asset"`
	Amount        string `json:"amount"`
}

type AggregatedBalanceChange struct {
	Account string `json:"account"`
	Asset   string `json:"asset"`
	Amount  string `json:"amount"`
}

func runBalanceChange(cmd *cobra.Command, args []string) error {
	client, err := aptos.NewClient(api.GetNetworkConfig())
	if err != nil {
		return fmt.Errorf("failed to create client: %w", err)
	}

	userTx, version, err := getTransaction(client, args)
	if err != nil {
		return err
	}

	storeInfo := extractTransferStoreInfoFromUserTx(userTx)
	events := buildBalanceChangeEvents(userTx, storeInfo, client, version)

	encoder := json.NewEncoder(os.Stdout)
	encoder.SetIndent("", "  ")

	if aggregateFlag {
		aggregated := aggregateEvents(events)
		if err := encoder.Encode(aggregated); err != nil {
			return fmt.Errorf("failed to encode response: %w", err)
		}
	} else {
		if err := encoder.Encode(events); err != nil {
			return fmt.Errorf("failed to encode response: %w", err)
		}
	}

	return nil
}

func buildBalanceChangeEvents(userTx *aptosapi.UserTransaction, storeInfo map[string]transferStoreMetadata, client *aptos.Client, version uint64) []BalanceChange {
	var events []BalanceChange

	// Gas fee entry
	gasFee := new(big.Int).Mul(
		new(big.Int).SetUint64(userTx.GasUsed),
		new(big.Int).SetUint64(userTx.GasUnitPrice),
	)
	if gasFee.Sign() > 0 {
		sender := userTx.Sender.String()
		aptStore := findSenderAPTStore(userTx, sender)
		events = append(events, BalanceChange{
			Type:          "gas_fee",
			Account:       sender,
			FungibleStore: aptStore,
			Asset:         "0xa",
			Amount:        gasFee.String(),
		})
	}

	// Withdraw/Deposit events
	for _, event := range userTx.Events {
		var eventType string
		switch event.Type {
		case "0x1::fungible_asset::Withdraw":
			eventType = "withdraw"
		case "0x1::fungible_asset::Deposit":
			eventType = "deposit"
		default:
			continue
		}

		store := getString(event.Data, "store")
		amount := getString(event.Data, "amount")

		meta, ok := storeInfo[store]
		if !ok {
			meta = queryTransferStoreInfo(client, store, version)
		}

		events = append(events, BalanceChange{
			Type:          eventType,
			Account:       meta.owner,
			FungibleStore: store,
			Asset:         meta.asset,
			Amount:        amount,
		})
	}

	return events
}

// findSenderAPTStore finds the sender's APT fungible store address from tx changes.
func findSenderAPTStore(userTx *aptosapi.UserTransaction, sender string) string {
	// Build owner map from ObjectCore changes
	owners := make(map[string]string)
	for _, change := range userTx.Changes {
		if change.Type != aptosapi.WriteSetChangeVariantWriteResource {
			continue
		}
		writeResource, ok := change.Inner.(*aptosapi.WriteSetChangeWriteResource)
		if !ok || writeResource.Data == nil {
			continue
		}
		if writeResource.Data.Type != "0x1::object::ObjectCore" {
			continue
		}
		owners[writeResource.Address.String()] = getString(writeResource.Data.Data, "owner")
	}

	// Find FungibleStore with owner=sender and asset=0xa
	for _, change := range userTx.Changes {
		if change.Type != aptosapi.WriteSetChangeVariantWriteResource {
			continue
		}
		writeResource, ok := change.Inner.(*aptosapi.WriteSetChangeWriteResource)
		if !ok || writeResource.Data == nil {
			continue
		}
		if !strings.Contains(writeResource.Data.Type, "fungible_asset::FungibleStore") {
			continue
		}
		address := writeResource.Address.String()
		asset := getString(writeResource.Data.Data, "metadata", "inner")
		if owners[address] == sender && asset == "0xa" {
			return address
		}
	}

	return ""
}

func aggregateEvents(events []BalanceChange) []AggregatedBalanceChange {
	type key struct {
		account string
		asset   string
	}
	totals := make(map[key]*big.Int)
	order := []key{}

	for _, e := range events {
		k := key{account: e.Account, asset: e.Asset}
		if _, exists := totals[k]; !exists {
			totals[k] = new(big.Int)
			order = append(order, k)
		}

		amount := new(big.Int)
		amount.SetString(e.Amount, 10)

		switch e.Type {
		case "withdraw", "gas_fee":
			totals[k].Sub(totals[k], amount)
		case "deposit":
			totals[k].Add(totals[k], amount)
		}
	}

	var result []AggregatedBalanceChange
	for _, k := range order {
		result = append(result, AggregatedBalanceChange{
			Account: k.account,
			Asset:   k.asset,
			Amount:  totals[k].String(),
		})
	}

	return result
}

// Shared helpers used by both balance_change.go and graph.go

type transferStoreMetadata struct {
	owner string
	asset string
}

func extractTransferStoreInfoFromUserTx(userTx *aptosapi.UserTransaction) map[string]transferStoreMetadata {
	info := make(map[string]transferStoreMetadata)

	// Extract owners from ObjectCore
	owners := make(map[string]string)
	for _, change := range userTx.Changes {
		if change.Type != aptosapi.WriteSetChangeVariantWriteResource {
			continue
		}

		writeResource, ok := change.Inner.(*aptosapi.WriteSetChangeWriteResource)
		if !ok || writeResource.Data == nil {
			continue
		}

		if writeResource.Data.Type != "0x1::object::ObjectCore" {
			continue
		}

		address := writeResource.Address.String()
		owner := getString(writeResource.Data.Data, "owner")
		owners[address] = owner
	}

	// Extract assets from FungibleStore
	for _, change := range userTx.Changes {
		if change.Type != aptosapi.WriteSetChangeVariantWriteResource {
			continue
		}

		writeResource, ok := change.Inner.(*aptosapi.WriteSetChangeWriteResource)
		if !ok || writeResource.Data == nil {
			continue
		}

		if writeResource.Data.Type != "0x1::fungible_asset::FungibleStore" {
			continue
		}

		address := writeResource.Address.String()
		asset := getString(writeResource.Data.Data, "metadata", "inner")

		info[address] = transferStoreMetadata{
			owner: owners[address],
			asset: asset,
		}
	}

	return info
}

func queryTransferStoreInfo(client *aptos.Client, store string, version uint64) transferStoreMetadata {
	meta := transferStoreMetadata{}

	addr := aptos.AccountAddress{}
	if err := addr.ParseStringRelaxed(store); err != nil {
		return meta
	}

	objCore, err := client.AccountResource(addr, "0x1::object::ObjectCore", version)
	if err == nil {
		meta.owner = getString(objCore, "data", "owner")
	}

	fsResource, err := client.AccountResource(addr, "0x1::fungible_asset::FungibleStore", version)
	if err == nil {
		meta.asset = getString(fsResource, "data", "metadata", "inner")
	}

	return meta
}
