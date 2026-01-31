package tx

import (
	"github.com/spf13/cobra"
)

var TxCmd = &cobra.Command{
	Use:   "tx",
	Short: "Transaction commands",
	Long:  `Commands for viewing and analyzing Aptos transactions.`,
}

func init() {
	TxCmd.AddCommand(viewCmd)
	TxCmd.AddCommand(balanceChangeCmd)
	TxCmd.AddCommand(transfersCmd)
	TxCmd.AddCommand(traceCmd)
}
