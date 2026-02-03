package account

import (
	"fmt"

	"github.com/0xbe1/aptly/pkg/api"
	"github.com/spf13/cobra"
)

var moduleCmd = &cobra.Command{
	Use:   "module <address> <module_name>",
	Short: "Get a specific account module",
	Long: `Fetches and displays a specific module for an account from the Aptos mainnet.

Examples:
  apt account module 0x1 coin
  apt account module 0x1 aptos_account`,
	Args: cobra.ExactArgs(2),
	RunE: runModule,
}

func runModule(cmd *cobra.Command, args []string) error {
	return api.GetAndPrint(fmt.Sprintf("%s/accounts/%s/module/%s", api.BaseURL, args[0], args[1]))
}
