package account

import (
	"fmt"

	"github.com/0xbe1/aptly/pkg/api"
	"github.com/spf13/cobra"
)

var AccountCmd = &cobra.Command{
	Use:   "account <address>",
	Short: "Get account information",
	Long:  `Fetches and displays account information (authentication key, sequence number) from the Aptos mainnet.`,
	Args:  cobra.ExactArgs(1),
	RunE:  runAccount,
}

func init() {
	AccountCmd.AddCommand(resourcesCmd)
	AccountCmd.AddCommand(resourceCmd)
	AccountCmd.AddCommand(modulesCmd)
	AccountCmd.AddCommand(moduleCmd)
	AccountCmd.AddCommand(balanceCmd)
	AccountCmd.AddCommand(txsCmd)
	AccountCmd.AddCommand(transfersCmd)
}

func runAccount(cmd *cobra.Command, args []string) error {
	return api.GetAndPrint(fmt.Sprintf("%s/accounts/%s", api.BaseURL, args[0]))
}
