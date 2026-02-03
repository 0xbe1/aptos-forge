package node

import (
	"github.com/0xbe1/aptly/pkg/api"
	"github.com/spf13/cobra"
)

var infoCmd = &cobra.Command{
	Use:   "info",
	Short: "Get node info",
	Long:  `Fetches and displays information about the Aptos node.`,
	Args:  cobra.NoArgs,
	RunE:  runInfo,
}

func runInfo(cmd *cobra.Command, args []string) error {
	return api.GetAndPrint(api.BaseURL + "/info")
}
