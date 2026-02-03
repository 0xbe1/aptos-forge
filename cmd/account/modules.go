package account

import (
	"fmt"

	"github.com/0xbe1/aptly/pkg/api"
	"github.com/spf13/cobra"
)

var modulesCmd = &cobra.Command{
	Use:   "modules <address>",
	Short: "List all account modules",
	Long:  `Fetches and displays all modules for an account from the Aptos mainnet.`,
	Args:  cobra.ExactArgs(1),
	RunE:  runModules,
}

func runModules(cmd *cobra.Command, args []string) error {
	return api.GetAndPrint(fmt.Sprintf("%s/accounts/%s/modules", api.BaseURL, args[0]))
}
