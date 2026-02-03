package account

import (
	"fmt"
	"net/url"

	"github.com/0xbe1/aptly/pkg/api"
	"github.com/spf13/cobra"
)

var resourceCmd = &cobra.Command{
	Use:   "resource <address> <resource_type>",
	Short: "Get a specific account resource",
	Long: `Fetches and displays a specific resource for an account from the Aptos mainnet.

Examples:
  apt account resource 0x1 0x1::account::Account
  apt account resource 0x1 0x1::coin::CoinStore<0x1::aptos_coin::AptosCoin>`,
	Args: cobra.ExactArgs(2),
	RunE: runResource,
}

func runResource(cmd *cobra.Command, args []string) error {
	return api.GetAndPrint(fmt.Sprintf("%s/accounts/%s/resource/%s", api.BaseURL, args[0], url.PathEscape(args[1])))
}
