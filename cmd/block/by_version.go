package block

import (
	"fmt"

	"github.com/0xbe1/aptly/pkg/api"
	"github.com/spf13/cobra"
)

var byVersionWithTransactions bool

var byVersionCmd = &cobra.Command{
	Use:   "by-version <version>",
	Short: "Get block by transaction version",
	Long: `Fetches and displays block information by transaction version from the Aptos mainnet.

Examples:
  apt block by-version 2658869495
  apt block by-version 2658869495 --with-transactions`,
	Args: cobra.ExactArgs(1),
	RunE: runByVersion,
}

func init() {
	byVersionCmd.Flags().BoolVar(&byVersionWithTransactions, "with-transactions", false, "Include transactions in the response")
}

func runByVersion(cmd *cobra.Command, args []string) error {
	return api.GetAndPrint(fmt.Sprintf("%s/blocks/by_version/%s?with_transactions=%t", api.BaseURL, args[0], byVersionWithTransactions))
}
