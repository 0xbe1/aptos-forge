package tx

import (
	"encoding/json"
	"fmt"
	"math/big"
	"os"
	"strconv"
	"strings"

	"github.com/aptos-labs/aptos-go-sdk"
	"github.com/aptos-labs/aptos-go-sdk/api"
	"github.com/spf13/cobra"
)

var balanceChangeCmd = &cobra.Command{
	Use:   "balance-change <tx_version>",
	Short: "Show balance changes in a transaction",
	Long:  `Analyzes FungibleStore balance changes between tx_version and tx_version-1.`,
	Args:  cobra.ExactArgs(1),
	RunE:  runBalanceChange,
}

type BalanceChange struct {
	Account       string `json:"account"`
	FungibleStore string `json:"fungible_store"`
	Asset         string `json:"asset"`
	BalanceBefore string `json:"balance_before"`
	BalanceAfter  string `json:"balance_after"`
	Change        string `json:"change"`
}

type fungibleStoreInfo struct {
	address   string
	owner     string
	assetType string
	balance   string
}

func runBalanceChange(cmd *cobra.Command, args []string) error {
	version, err := strconv.ParseUint(args[0], 10, 64)
	if err != nil {
		return fmt.Errorf("invalid transaction version: %w", err)
	}

	if version == 0 {
		return fmt.Errorf("cannot get balance change for version 0")
	}

	client, err := aptos.NewClient(aptos.MainnetConfig)
	if err != nil {
		return fmt.Errorf("failed to create client: %w", err)
	}

	// Fetch current transaction
	txCurrent, err := client.TransactionByVersion(version)
	if err != nil {
		return fmt.Errorf("failed to fetch transaction %d: %w", version, err)
	}

	// Extract FungibleStore changes from current transaction
	stores := extractFungibleStores(txCurrent)

	// For each store, query the previous balance at version-1
	changes := []BalanceChange{}
	for _, store := range stores {
		addr := aptos.AccountAddress{}
		if err := addr.ParseStringRelaxed(store.address); err != nil {
			continue
		}

		prevBalance := "0"
		prevResource, err := client.AccountResource(addr, "0x1::fungible_asset::FungibleStore", version-1)
		if err == nil {
			prevBalance = getString(prevResource, "data", "balance")
		}

		change := calculateChange(prevBalance, store.balance)
		if change == "0" {
			continue
		}
		changes = append(changes, BalanceChange{
			Account:       store.owner,
			FungibleStore: store.address,
			Asset:         store.assetType,
			BalanceBefore: prevBalance,
			BalanceAfter:  store.balance,
			Change:        change,
		})
	}

	encoder := json.NewEncoder(os.Stdout)
	encoder.SetIndent("", "  ")
	if err := encoder.Encode(changes); err != nil {
		return fmt.Errorf("failed to encode response: %w", err)
	}

	return nil
}

func extractFungibleStores(tx *api.CommittedTransaction) []fungibleStoreInfo {
	var stores []fungibleStoreInfo

	userTx, err := tx.UserTransaction()
	if err != nil {
		return stores
	}

	// First pass: extract ObjectCore owners (address -> owner)
	owners := make(map[string]string)
	for _, change := range userTx.Changes {
		if change.Type != api.WriteSetChangeVariantWriteResource {
			continue
		}

		writeResource, ok := change.Inner.(*api.WriteSetChangeWriteResource)
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

	// Second pass: extract FungibleStores
	for _, change := range userTx.Changes {
		if change.Type != api.WriteSetChangeVariantWriteResource {
			continue
		}

		writeResource, ok := change.Inner.(*api.WriteSetChangeWriteResource)
		if !ok || writeResource.Data == nil {
			continue
		}

		if !strings.Contains(writeResource.Data.Type, "fungible_asset::FungibleStore") {
			continue
		}

		address := writeResource.Address.String()
		balance := getString(writeResource.Data.Data, "balance")
		metadataInner := getString(writeResource.Data.Data, "metadata", "inner")

		stores = append(stores, fungibleStoreInfo{
			address:   address,
			owner:     owners[address],
			assetType: metadataInner,
			balance:   balance,
		})
	}

	return stores
}

func calculateChange(before, after string) string {
	beforeBig := new(big.Int)
	afterBig := new(big.Int)

	beforeBig.SetString(before, 10)
	afterBig.SetString(after, 10)

	change := new(big.Int).Sub(afterBig, beforeBig)
	return change.String()
}
