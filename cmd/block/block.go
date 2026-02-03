package block

import (
	"fmt"

	"github.com/0xbe1/aptly/pkg/api"
	"github.com/spf13/cobra"
)

var withTransactions bool

var BlockCmd = &cobra.Command{
	Use:   "block <height>",
	Short: "Get block by height",
	Long: `Fetches and displays block information by height from the Aptos mainnet.

Examples:
  apt block 1000000
  apt block 1000000 --with-transactions`,
	Args: cobra.ExactArgs(1),
	RunE: runBlock,
}

func init() {
	BlockCmd.AddCommand(byVersionCmd)
	BlockCmd.Flags().BoolVar(&withTransactions, "with-transactions", false, "Include transactions in the response")
}

func runBlock(cmd *cobra.Command, args []string) error {
	return api.GetAndPrint(fmt.Sprintf("%s/blocks/by_height/%s?with_transactions=%t", api.BaseURL, args[0], withTransactions))
}
