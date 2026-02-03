package account

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

var (
	sendsLimit  uint64
	sendsPretty bool
)

var sendsCmd = &cobra.Command{
	Use:   "sends <address>",
	Short: "List outgoing asset transfers sent by an account",
	Long: `Fetches and displays outgoing asset transfers sent by an account from the Aptos mainnet.

Notes:
  - Only shows transactions where the account is the sender. Incoming transfers
    are not included because they are initiated by other accounts.
  - The --limit flag limits the number of account transactions scanned, not the
    number of sends returned. Fewer sends may be returned if not all transactions
    are asset transfers.

Detects transfers from:
  - 0x1::aptos_account::transfer_coins
  - 0x1::primary_fungible_store::transfer
  - 0x1::coin::transfer

Examples:
  aptly account sends 0x1
  aptly account sends 0x1 --limit 10
  aptly account sends 0x1 --limit 5 --pretty`,
	Args: cobra.ExactArgs(1),
	RunE: runSends,
}

func init() {
	sendsCmd.Flags().Uint64Var(&sendsLimit, "limit", 25, "Maximum number of account transactions to scan (not sends returned)")
	sendsCmd.Flags().BoolVar(&sendsPretty, "pretty", false, "Output in simple line format")
}

// Transfer represents a single asset transfer
type Transfer struct {
	From    string `json:"from"`
	To      string `json:"to"`
	Amount  string `json:"amount"`
	Asset   string `json:"asset"`
	Version uint64 `json:"version"`
}

// assetMetadata holds cached symbol and decimals for an asset
type assetMetadata struct {
	symbol   string
	decimals uint8
}

// metadataCache caches asset metadata to avoid repeated queries
var metadataCache = make(map[string]assetMetadata)

func runSends(cmd *cobra.Command, args []string) error {
	client, err := aptos.NewClient(api.GetNetworkConfig())
	if err != nil {
		return fmt.Errorf("failed to create client: %w", err)
	}

	var addr aptos.AccountAddress
	if err := addr.ParseStringRelaxed(args[0]); err != nil {
		return fmt.Errorf("invalid address: %w", err)
	}

	// Fetch transactions using SDK
	start := uint64(0)
	txs, err := client.AccountTransactions(addr, &start, &sendsLimit)
	if err != nil {
		return fmt.Errorf("failed to fetch transactions: %w", err)
	}

	// Filter and extract transfers
	transfers := []Transfer{}
	for _, tx := range txs {
		userTx, err := tx.UserTransaction()
		if err != nil {
			continue // Skip non-user transactions
		}
		if transfer, ok := extractTransfer(client, userTx); ok {
			transfers = append(transfers, transfer)
		}
	}

	// Output
	if sendsPretty {
		printPrettySends(transfers)
		return nil
	}

	encoder := json.NewEncoder(os.Stdout)
	encoder.SetIndent("", "  ")
	return encoder.Encode(transfers)
}

func printPrettySends(transfers []Transfer) {
	// Calculate max widths for alignment
	maxAmountLen := 0
	maxAssetLen := 0
	for _, t := range transfers {
		if len(t.Amount) > maxAmountLen {
			maxAmountLen = len(t.Amount)
		}
		if len(t.Asset) > maxAssetLen {
			maxAssetLen = len(t.Asset)
		}
	}

	for _, t := range transfers {
		fmt.Printf("[%d] %*s %-*s → %s\n", t.Version, maxAmountLen, t.Amount, maxAssetLen, t.Asset, t.To)
	}
}

// extractTransfer attempts to extract transfer info from a user transaction
func extractTransfer(client *aptos.Client, userTx *aptosapi.UserTransaction) (Transfer, bool) {
	if userTx.Payload == nil || userTx.Payload.Type != aptosapi.TransactionPayloadVariantEntryFunction {
		return Transfer{}, false
	}

	entryFunc, ok := userTx.Payload.Inner.(*aptosapi.TransactionPayloadEntryFunction)
	if !ok || entryFunc == nil {
		return Transfer{}, false
	}

	function := entryFunc.Function
	args := entryFunc.Arguments
	typeArgs := entryFunc.TypeArguments

	var to, amountStr, asset string
	var isFungibleAsset bool

	switch function {
	case "0x1::aptos_account::transfer_coins":
		if len(args) < 2 || len(typeArgs) < 1 {
			return Transfer{}, false
		}
		to = getString(args[0])
		amountStr = getString(args[1])
		asset = typeArgs[0]

	case "0x1::primary_fungible_store::transfer":
		if len(args) < 3 {
			return Transfer{}, false
		}
		// args: [{inner: metadata_addr}, to, amount]
		to = getString(args[1])
		amountStr = getString(args[2])
		asset = getInnerString(args[0])
		isFungibleAsset = true

	case "0x1::coin::transfer":
		if len(args) < 2 || len(typeArgs) < 1 {
			return Transfer{}, false
		}
		to = getString(args[0])
		amountStr = getString(args[1])
		asset = typeArgs[0]

	default:
		return Transfer{}, false
	}

	// Get asset metadata (symbol and decimals)
	meta := getAssetMetadata(client, asset, isFungibleAsset)

	return Transfer{
		From:    userTx.Sender.String(),
		To:      to,
		Amount:  formatAmount(amountStr, meta.decimals),
		Asset:   meta.symbol,
		Version: userTx.Version,
	}, true
}

// getAssetMetadata returns the metadata for an asset, using cache when available
func getAssetMetadata(client *aptos.Client, asset string, isFungibleAsset bool) assetMetadata {
	if cached, ok := metadataCache[asset]; ok {
		return cached
	}

	var meta assetMetadata
	if isFungibleAsset {
		meta = queryFungibleAssetMetadata(client, asset)
	} else {
		meta = queryCoinMetadata(client, asset)
	}

	metadataCache[asset] = meta
	return meta
}

// queryFungibleAssetMetadata queries symbol and decimals for a fungible asset
func queryFungibleAssetMetadata(client *aptos.Client, metadataAddr string) assetMetadata {
	meta := assetMetadata{symbol: shortenAddr(metadataAddr), decimals: 0}

	var addr aptos.AccountAddress
	if err := addr.ParseStringRelaxed(metadataAddr); err != nil {
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
		if symbol, ok := result[0].(string); ok && symbol != "" {
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

// queryCoinMetadata queries symbol and decimals for a coin type
func queryCoinMetadata(client *aptos.Client, coinType string) assetMetadata {
	// Fast path for APT
	if coinType == "0x1::aptos_coin::AptosCoin" {
		return assetMetadata{symbol: "APT", decimals: 8}
	}

	meta := assetMetadata{symbol: shortenAddr(coinType), decimals: 0}

	// Parse coin type to get the type tag
	typeTag, err := aptos.ParseTypeTag(coinType)
	if err != nil {
		return meta
	}

	// Query symbol using 0x1::coin::symbol<CoinType>
	symbolPayload := &aptos.ViewPayload{
		Module:   aptos.ModuleId{Address: aptos.AccountOne, Name: "coin"},
		Function: "symbol",
		ArgTypes: []aptos.TypeTag{*typeTag},
		Args:     [][]byte{},
	}
	if result, err := client.View(symbolPayload); err == nil && len(result) > 0 {
		if symbol, ok := result[0].(string); ok && symbol != "" {
			meta.symbol = symbol
		}
	}

	// Query decimals using 0x1::coin::decimals<CoinType>
	decimalsPayload := &aptos.ViewPayload{
		Module:   aptos.ModuleId{Address: aptos.AccountOne, Name: "coin"},
		Function: "decimals",
		ArgTypes: []aptos.TypeTag{*typeTag},
		Args:     [][]byte{},
	}
	if result, err := client.View(decimalsPayload); err == nil && len(result) > 0 {
		if decimals, ok := result[0].(float64); ok {
			meta.decimals = uint8(decimals)
		}
	}

	return meta
}

// formatAmount divides the raw amount by 10^decimals and formats it
func formatAmount(amountStr string, decimals uint8) string {
	if decimals == 0 {
		return amountStr
	}

	amount := new(big.Int)
	if _, ok := amount.SetString(amountStr, 10); !ok {
		return amountStr
	}

	divisor := new(big.Int).Exp(big.NewInt(10), big.NewInt(int64(decimals)), nil)

	// Get integer and fractional parts
	intPart := new(big.Int).Div(amount, divisor)
	fracPart := new(big.Int).Mod(amount, divisor)

	// Format fractional part with leading zeros
	fracStr := fmt.Sprintf("%0*s", decimals, fracPart.String())
	// Trim trailing zeros
	fracStr = strings.TrimRight(fracStr, "0")

	if fracStr == "" {
		return intPart.String()
	}
	return fmt.Sprintf("%s.%s", intPart.String(), fracStr)
}

// getString extracts a string from an interface
func getString(v any) string {
	if s, ok := v.(string); ok {
		return s
	}
	return ""
}

// getInnerString extracts the "inner" field from a map, or returns the string directly
func getInnerString(v any) string {
	if m, ok := v.(map[string]any); ok {
		if inner, ok := m["inner"].(string); ok {
			return inner
		}
	}
	return getString(v)
}

// shortenAddr shortens an address or type for display
func shortenAddr(addr string) string {
	if len(addr) > 12 {
		return addr[:6] + "..." + addr[len(addr)-4:]
	}
	return addr
}

