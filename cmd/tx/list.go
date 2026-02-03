package tx

import (
	"fmt"

	"github.com/0xbe1/aptly/pkg/api"
	"github.com/spf13/cobra"
)

var (
	listLimit uint64
	listStart uint64
)

var listCmd = &cobra.Command{
	Use:   "list",
	Short: "List recent transactions",
	Long: `Fetches and displays recent transactions from the Aptos mainnet.

Examples:
  apt tx list
  apt tx list --limit 10
  apt tx list --limit 10 --start 2658869495`,
	Args: cobra.NoArgs,
	RunE: runList,
}

func init() {
	TxCmd.AddCommand(listCmd)
	listCmd.Flags().Uint64Var(&listLimit, "limit", 25, "Maximum number of transactions to return")
	listCmd.Flags().Uint64Var(&listStart, "start", 0, "Starting version (0 means start from the latest)")
}

func runList(cmd *cobra.Command, args []string) error {
	url := fmt.Sprintf("%s/transactions?limit=%d", api.BaseURL, listLimit)
	if listStart > 0 {
		url = fmt.Sprintf("%s&start=%d", url, listStart)
	}
	return api.GetAndPrint(url)
}
