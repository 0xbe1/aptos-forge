package account

import (
	"fmt"

	"github.com/0xbe1/aptly/pkg/api"
	"github.com/spf13/cobra"
)

var (
	txsLimit uint64
	txsStart uint64
)

var txsCmd = &cobra.Command{
	Use:   "txs <address>",
	Short: "List account transactions",
	Long: `Fetches and displays transactions for an account from the Aptos mainnet.

Examples:
  apt account txs 0x1
  apt account txs 0x1 --limit 10
  apt account txs 0x1 --limit 10 --start 100`,
	Args: cobra.ExactArgs(1),
	RunE: runTxs,
}

func init() {
	txsCmd.Flags().Uint64Var(&txsLimit, "limit", 25, "Maximum number of transactions to return")
	txsCmd.Flags().Uint64Var(&txsStart, "start", 0, "Starting sequence number (0 means start from the beginning)")
}

func runTxs(cmd *cobra.Command, args []string) error {
	url := fmt.Sprintf("%s/accounts/%s/transactions?limit=%d", api.BaseURL, args[0], txsLimit)
	if txsStart > 0 {
		url = fmt.Sprintf("%s&start=%d", url, txsStart)
	}
	return api.GetAndPrint(url)
}
