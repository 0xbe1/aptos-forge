package view

import (
	"encoding/json"
	"fmt"

	"github.com/0xbe1/aptly/pkg/api"
	"github.com/spf13/cobra"
)

var (
	typeArgs []string
	args     []string
)

var ViewCmd = &cobra.Command{
	Use:   "view <function>",
	Short: "Execute a view function",
	Long: `Executes a view function on the Aptos mainnet and returns the result.

View functions are read-only functions that can be called without a transaction.

Examples:
  apt view 0x1::coin::balance --type-args 0x1::aptos_coin::AptosCoin --args '"0x1"'
  apt view 0x1::account::exists_at --args '"0x1"'
  apt view 0x1::aptos_coin::supply`,
	Args: cobra.ExactArgs(1),
	RunE: runView,
}

func init() {
	ViewCmd.Flags().StringArrayVar(&typeArgs, "type-args", []string{}, "Type arguments for the function")
	ViewCmd.Flags().StringArrayVar(&args, "args", []string{}, "Arguments for the function (JSON encoded)")
}

func runView(cmd *cobra.Command, cmdArgs []string) error {
	var parsedArgs []any
	for _, arg := range args {
		var value any
		if err := json.Unmarshal([]byte(arg), &value); err != nil {
			return fmt.Errorf("failed to parse argument %q as JSON: %w", arg, err)
		}
		parsedArgs = append(parsedArgs, value)
	}

	body := map[string]any{
		"function":       cmdArgs[0],
		"type_arguments": typeArgs,
		"arguments":      parsedArgs,
	}

	return api.PostAndPrint(api.BaseURL+"/view", body)
}
