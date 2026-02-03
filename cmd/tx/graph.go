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
	"github.com/aptos-labs/aptos-go-sdk/bcs"
	"github.com/spf13/cobra"
)

var prettyOutput bool

var graphCmd = &cobra.Command{
	Use:   "graph [version_or_hash]",
	Short: "Show asset transfers as a graph",
	Long:  `Pairs withdraw→deposit events for the same asset to show transfer flows.`,
	Args:  cobra.MaximumNArgs(1),
	RunE:  runGraph,
}

func init() {
	graphCmd.Flags().BoolVar(&prettyOutput, "pretty", false, "Human-readable output")
}

type Transfer struct {
	From   string `json:"from"`
	To     string `json:"to"`
	Asset  string `json:"asset"`
	Amount string `json:"amount"`
}

type OrphanEvent struct {
	Account string `json:"account"`
	Asset   string `json:"asset"`
	Amount  string `json:"amount"`
}

type Orphans struct {
	In  []OrphanEvent `json:"in"`
	Out []OrphanEvent `json:"out"`
}

type TransferGraph struct {
	Transfers []Transfer `json:"transfers"`
	Orphans   Orphans    `json:"orphans"`
}

func runGraph(cmd *cobra.Command, args []string) error {
	client, err := aptos.NewClient(api.GetNetworkConfig())
	if err != nil {
		return fmt.Errorf("failed to create client: %w", err)
	}

	userTx, version, err := getTransaction(client, args)
	if err != nil {
		return err
	}

	storeInfo := extractTransferStoreInfoFromUserTx(userTx)
	graph := buildTransferGraph(userTx, storeInfo, client, version)

	if prettyOutput {
		printPrettyGraph(graph, client)
		return nil
	}

	encoder := json.NewEncoder(os.Stdout)
	encoder.SetIndent("", "  ")
	if err := encoder.Encode(graph); err != nil {
		return fmt.Errorf("failed to encode response: %w", err)
	}

	return nil
}

type pendingWithdraw struct {
	account string
	amount  string
}

func buildTransferGraph(userTx *aptosapi.UserTransaction, storeInfo map[string]transferStoreMetadata, client *aptos.Client, version uint64) TransferGraph {
	graph := TransferGraph{
		Transfers: []Transfer{},
		Orphans: Orphans{
			In:  []OrphanEvent{},
			Out: []OrphanEvent{},
		},
	}

	// Track pending withdraws by asset: asset -> list of pending withdraws
	pendingWithdraws := make(map[string][]pendingWithdraw)

	for _, event := range userTx.Events {
		store := getString(event.Data, "store")
		amount := getString(event.Data, "amount")

		meta, ok := storeInfo[store]
		if !ok {
			meta = queryTransferStoreInfo(client, store, version)
		}

		switch event.Type {
		case "0x1::fungible_asset::Withdraw":
			pendingWithdraws[meta.asset] = append(pendingWithdraws[meta.asset], pendingWithdraw{
				account: meta.owner,
				amount:  amount,
			})

		case "0x1::fungible_asset::Deposit":
			pending := pendingWithdraws[meta.asset]
			if len(pending) > 0 {
				// Match with first pending withdraw for this asset
				withdraw := pending[0]
				pendingWithdraws[meta.asset] = pending[1:]

				graph.Transfers = append(graph.Transfers, Transfer{
					From:   withdraw.account,
					To:     meta.owner,
					Asset:  meta.asset,
					Amount: amount,
				})
			} else {
				// No matching withdraw - orphan in
				graph.Orphans.In = append(graph.Orphans.In, OrphanEvent{
					Account: meta.owner,
					Asset:   meta.asset,
					Amount:  amount,
				})
			}
		}
	}

	// Any remaining pending withdraws are orphan outs
	for asset, pending := range pendingWithdraws {
		for _, w := range pending {
			graph.Orphans.Out = append(graph.Orphans.Out, OrphanEvent{
				Account: w.account,
				Asset:   asset,
				Amount:  w.amount,
			})
		}
	}

	return graph
}

type assetMetadata struct {
	symbol   string
	decimals uint8
}

func queryAssetMetadata(client *aptos.Client, asset string) assetMetadata {
	meta := assetMetadata{symbol: "", decimals: 0}

	addr := aptos.AccountAddress{}
	if err := addr.ParseStringRelaxed(asset); err != nil {
		return meta
	}

	addrBytes, err := bcs.Serialize(&addr)
	if err != nil {
		return meta
	}

	// Type argument: 0x1::fungible_asset::Metadata
	metadataTypeTag := aptos.NewTypeTag(&aptos.StructTag{
		Address:    aptos.AccountOne,
		Module:     "fungible_asset",
		Name:       "Metadata",
		TypeParams: []aptos.TypeTag{},
	})

	// Query symbol
	symbolPayload := &aptos.ViewPayload{
		Module:   aptos.ModuleId{Address: aptos.AccountOne, Name: "fungible_asset"},
		Function: "symbol",
		ArgTypes: []aptos.TypeTag{metadataTypeTag},
		Args:     [][]byte{addrBytes},
	}
	if result, err := client.View(symbolPayload); err == nil && len(result) > 0 {
		if symbol, ok := result[0].(string); ok {
			meta.symbol = symbol
		}
	}

	// Query decimals
	decimalsPayload := &aptos.ViewPayload{
		Module:   aptos.ModuleId{Address: aptos.AccountOne, Name: "fungible_asset"},
		Function: "decimals",
		ArgTypes: []aptos.TypeTag{metadataTypeTag},
		Args:     [][]byte{addrBytes},
	}
	if result, err := client.View(decimalsPayload); err == nil && len(result) > 0 {
		if decimals, ok := result[0].(float64); ok {
			meta.decimals = uint8(decimals)
		}
	}

	return meta
}

func formatAmount(amount string, decimals uint8) string {
	if decimals == 0 {
		return amount
	}

	// Parse the amount as a big integer
	val := new(big.Int)
	if _, ok := val.SetString(amount, 10); !ok {
		return amount
	}

	// Calculate divisor (10^decimals)
	divisor := new(big.Int).Exp(big.NewInt(10), big.NewInt(int64(decimals)), nil)

	// Get integer and fractional parts
	intPart := new(big.Int).Div(val, divisor)
	fracPart := new(big.Int).Mod(val, divisor)

	// Format fractional part with leading zeros
	fracStr := fmt.Sprintf("%0*s", decimals, fracPart.String())
	// Trim trailing zeros
	fracStr = strings.TrimRight(fracStr, "0")

	if fracStr == "" {
		return intPart.String()
	}
	return fmt.Sprintf("%s.%s", intPart.String(), fracStr)
}

func formatAsset(asset string, meta assetMetadata) string {
	if meta.symbol != "" {
		return fmt.Sprintf("%s (%s)", meta.symbol, truncateAddress(asset))
	}
	return truncateAddress(asset)
}

func printPrettyGraph(graph TransferGraph, client *aptos.Client) {
	// Collect all unique assets
	assets := make(map[string]bool)
	for _, t := range graph.Transfers {
		assets[t.Asset] = true
	}
	for _, o := range graph.Orphans.In {
		assets[o.Asset] = true
	}
	for _, o := range graph.Orphans.Out {
		assets[o.Asset] = true
	}

	// Fetch metadata for all assets
	assetMeta := make(map[string]assetMetadata)
	for asset := range assets {
		assetMeta[asset] = queryAssetMetadata(client, asset)
	}

	// Group transfers by sender
	bySender := make(map[string][]Transfer)
	for _, t := range graph.Transfers {
		bySender[t.From] = append(bySender[t.From], t)
	}

	for sender, transfers := range bySender {
		fmt.Println(truncateAddress(sender))
		for _, t := range transfers {
			meta := assetMeta[t.Asset]
			fmt.Printf("  → %s   %s %s\n",
				truncateAddress(t.To),
				formatAmount(t.Amount, meta.decimals),
				formatAsset(t.Asset, meta))
		}
		fmt.Println()
	}

	if len(graph.Orphans.In) > 0 || len(graph.Orphans.Out) > 0 {
		fmt.Println("Orphans:")
		for _, o := range graph.Orphans.In {
			meta := assetMeta[o.Asset]
			fmt.Printf("  IN:  %s  %s %s\n",
				truncateAddress(o.Account),
				formatAmount(o.Amount, meta.decimals),
				formatAsset(o.Asset, meta))
		}
		for _, o := range graph.Orphans.Out {
			meta := assetMeta[o.Asset]
			fmt.Printf("  OUT: %s  %s %s\n",
				truncateAddress(o.Account),
				formatAmount(o.Amount, meta.decimals),
				formatAsset(o.Asset, meta))
		}
	}
}

func truncateAddress(addr string) string {
	if len(addr) <= 12 {
		return addr
	}
	return addr[:6] + ".." + addr[len(addr)-4:]
}
