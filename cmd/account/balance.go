package account

import (
	"fmt"
	"net/url"

	"github.com/0xbe1/aptly/pkg/api"
	"github.com/spf13/cobra"
)

var balanceCmd = &cobra.Command{
	Use:   "balance <address> [asset_type]",
	Short: "Get account balance",
	Long: `Fetches and displays the balance for an account from the Aptos mainnet.

If no asset type is specified, defaults to APT (0x1::aptos_coin::AptosCoin).

Examples:
  apt account balance 0x1
  apt account balance 0x1 0x1::aptos_coin::AptosCoin`,
	Args: cobra.RangeArgs(1, 2),
	RunE: runBalance,
}

func runBalance(cmd *cobra.Command, args []string) error {
	assetType := "0x1::aptos_coin::AptosCoin"
	if len(args) > 1 {
		assetType = args[1]
	}
	return api.GetAndPrint(fmt.Sprintf("%s/accounts/%s/balance/%s", api.BaseURL, args[0], url.PathEscape(assetType)))
}
