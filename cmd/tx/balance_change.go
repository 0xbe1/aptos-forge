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

var balanceChangeCmd = &cobra.Command{
	Use:   "balance-change [version_or_hash]",
	Short: "Show balance changes in a transaction",
	Long:  `Analyzes FungibleStore balance changes between the transaction and the previous version.`,
	Args:  cobra.MaximumNArgs(1),
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
	client, err := aptos.NewClient(api.GetNetworkConfig())
	if err != nil {
		return fmt.Errorf("failed to create client: %w", err)
	}

	userTx, version, err := getTransaction(client, args)
	if err != nil {
		return err
	}

	// Extract FungibleStore changes from current transaction
	stores := extractFungibleStoresFromUserTx(userTx)

	// For each store, query the previous balance
	// For simulated txs (version=0), query current state
	// For committed txs, query at version-1
	changes := []BalanceChange{}
	for _, store := range stores {
		addr := aptos.AccountAddress{}
		if err := addr.ParseStringRelaxed(store.address); err != nil {
			continue
		}

		prevBalance := "0"
		if version == 0 {
			// Simulated transaction: query current ledger state
			prevResource, err := client.AccountResource(addr, "0x1::fungible_asset::FungibleStore")
			if err == nil {
				prevBalance = getString(prevResource, "data", "balance")
			}
		} else {
			// Committed transaction: query at version-1
			prevResource, err := client.AccountResource(addr, "0x1::fungible_asset::FungibleStore", version-1)
			if err == nil {
				prevBalance = getString(prevResource, "data", "balance")
			}
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

func extractFungibleStoresFromUserTx(userTx *aptosapi.UserTransaction) []fungibleStoreInfo {
	var stores []fungibleStoreInfo

	// First pass: extract ObjectCore owners (address -> owner)
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

	// Second pass: extract FungibleStores
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
