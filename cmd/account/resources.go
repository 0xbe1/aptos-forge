package account

import (
	"fmt"

	"github.com/0xbe1/aptly/pkg/api"
	"github.com/spf13/cobra"
)

var resourcesCmd = &cobra.Command{
	Use:   "resources <address>",
	Short: "List all account resources",
	Long:  `Fetches and displays all resources for an account from the Aptos mainnet.`,
	Args:  cobra.ExactArgs(1),
	RunE:  runResources,
}

func runResources(cmd *cobra.Command, args []string) error {
	return api.GetAndPrint(fmt.Sprintf("%s/accounts/%s/resources", api.BaseURL, args[0]))
}
