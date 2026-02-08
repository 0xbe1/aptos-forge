package tx

import (
	"fmt"
	"strconv"

	"github.com/0xbe1/aptly/pkg/api"
	"github.com/spf13/cobra"
)

var TxCmd = &cobra.Command{
	Use:   "tx <version_or_hash>",
	Short: "Transaction commands",
	Long:  `View and analyze Aptos transactions. Run with a version or hash to view the raw transaction.`,
	Args:  cobra.ExactArgs(1),
	RunE:  runTx,
}

func init() {
	TxCmd.AddCommand(balanceChangeCmd)
	TxCmd.AddCommand(traceCmd)
	TxCmd.AddCommand(graphCmd)
	TxCmd.AddCommand(simulateCmd)
}

func runTx(cmd *cobra.Command, args []string) error {
	var url string
	if _, err := strconv.ParseUint(args[0], 10, 64); err == nil {
		url = fmt.Sprintf("%s/transactions/by_version/%s", api.BaseURL, args[0])
	} else {
		url = fmt.Sprintf("%s/transactions/by_hash/%s", api.BaseURL, args[0])
	}
	return api.GetAndPrint(url)
}
