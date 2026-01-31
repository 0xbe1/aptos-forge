package tx

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/aptos-labs/aptos-go-sdk"
	"github.com/aptos-labs/aptos-go-sdk/api"
	"github.com/spf13/cobra"
)

var transfersCmd = &cobra.Command{
	Use:   "transfers <version_or_hash>",
	Short: "Show asset transfers in a transaction",
	Long:  `Lists Withdraw/Deposit events from a transaction.`,
	Args:  cobra.ExactArgs(1),
	RunE:  runTransfers,
}

type TransferEvent struct {
	Type          string `json:"type"` // "withdraw" or "deposit"
	Account       string `json:"account"`
	FungibleStore string `json:"fungible_store"`
	Asset         string `json:"asset"`
	Amount        string `json:"amount"`
}

func runTransfers(cmd *cobra.Command, args []string) error {
	client, err := aptos.NewClient(aptos.MainnetConfig)
	if err != nil {
		return fmt.Errorf("failed to create client: %w", err)
	}

	userTx, version, err := fetchTransaction(client, args[0])
	if err != nil {
		return err
	}

	// Extract store info from tx changes
	storeInfo := extractTransferStoreInfoFromUserTx(userTx)

	// Extract flow events
	events := extractTransferEventsFromUserTx(userTx, storeInfo, client, version)

	encoder := json.NewEncoder(os.Stdout)
	encoder.SetIndent("", "  ")
	if err := encoder.Encode(events); err != nil {
		return fmt.Errorf("failed to encode response: %w", err)
	}

	return nil
}

type transferStoreMetadata struct {
	owner string
	asset string
}

func extractTransferStoreInfoFromUserTx(userTx *api.UserTransaction) map[string]transferStoreMetadata {
	info := make(map[string]transferStoreMetadata)

	// Extract owners from ObjectCore
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

	// Extract assets from FungibleStore
	for _, change := range userTx.Changes {
		if change.Type != api.WriteSetChangeVariantWriteResource {
			continue
		}

		writeResource, ok := change.Inner.(*api.WriteSetChangeWriteResource)
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

func extractTransferEventsFromUserTx(userTx *api.UserTransaction, storeInfo map[string]transferStoreMetadata, client *aptos.Client, version uint64) []TransferEvent {
	var result []TransferEvent

	for _, event := range userTx.Events {
		var transferType string
		switch event.Type {
		case "0x1::fungible_asset::Withdraw":
			transferType = "withdraw"
		case "0x1::fungible_asset::Deposit":
			transferType = "deposit"
		default:
			continue
		}

		store := getString(event.Data, "store")
		amount := getString(event.Data, "amount")

		meta, ok := storeInfo[store]
		if !ok {
			meta = queryTransferStoreInfo(client, store, version)
		}

		result = append(result, TransferEvent{
			Type:          transferType,
			Account:       meta.owner,
			FungibleStore: store,
			Asset:         meta.asset,
			Amount:        amount,
		})
	}

	return result
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
