package table

import (
	"encoding/json"
	"fmt"

	"github.com/0xbe1/aptly/pkg/api"
	"github.com/spf13/cobra"
)

var (
	keyType   string
	valueType string
	key       string
)

var TableCmd = &cobra.Command{
	Use:   "table",
	Short: "Table operations",
	Long:  `Commands for querying table data from the Aptos mainnet.`,
}

var itemCmd = &cobra.Command{
	Use:   "item <table_handle>",
	Short: "Get table item",
	Long: `Fetches a table item by its key from the Aptos mainnet.

Examples:
  apt table item 0x1b854694ae746cdbd8d44186ca4929b2b337df21d1c74633be19b2710552fdca \
    --key-type address \
    --value-type "0x1::staking_contract::StakingContract" \
    --key '"0x1"'`,
	Args: cobra.ExactArgs(1),
	RunE: runItem,
}

func init() {
	TableCmd.AddCommand(itemCmd)
	itemCmd.Flags().StringVar(&keyType, "key-type", "", "Type of the table key (required)")
	itemCmd.Flags().StringVar(&valueType, "value-type", "", "Type of the table value (required)")
	itemCmd.Flags().StringVar(&key, "key", "", "Key to look up (JSON encoded, required)")
	itemCmd.MarkFlagRequired("key-type")
	itemCmd.MarkFlagRequired("value-type")
	itemCmd.MarkFlagRequired("key")
}

func runItem(cmd *cobra.Command, args []string) error {
	var keyValue any
	if err := json.Unmarshal([]byte(key), &keyValue); err != nil {
		return fmt.Errorf("failed to parse key as JSON: %w", err)
	}

	body := map[string]any{
		"key_type":   keyType,
		"value_type": valueType,
		"key":        keyValue,
	}

	return api.PostAndPrint(fmt.Sprintf("%s/tables/%s/item", api.BaseURL, args[0]), body)
}
