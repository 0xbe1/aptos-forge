package node

import (
	"github.com/0xbe1/aptly/pkg/api"
	"github.com/spf13/cobra"
)

var healthCmd = &cobra.Command{
	Use:   "health",
	Short: "Check node health",
	Long:  `Checks and displays the health status of the Aptos node.`,
	Args:  cobra.NoArgs,
	RunE:  runHealth,
}

func runHealth(cmd *cobra.Command, args []string) error {
	return api.GetAndPrint(api.BaseURL + "/-/healthy")
}
