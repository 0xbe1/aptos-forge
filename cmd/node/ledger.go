package node

import (
	"github.com/0xbe1/aptly/pkg/api"
	"github.com/spf13/cobra"
)

var ledgerCmd = &cobra.Command{
	Use:   "ledger",
	Short: "Get ledger information",
	Long:  `Fetches and displays the current ledger information from the Aptos mainnet.`,
	Args:  cobra.NoArgs,
	RunE:  runLedger,
}

func runLedger(cmd *cobra.Command, args []string) error {
	return api.GetAndPrint(api.BaseURL + "/")
}
