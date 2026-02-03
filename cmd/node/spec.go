package node

import (
	"github.com/0xbe1/aptly/pkg/api"
	"github.com/spf13/cobra"
)

var specCmd = &cobra.Command{
	Use:   "spec",
	Short: "Get OpenAPI specification",
	Long:  `Fetches and displays the OpenAPI specification for the Aptos node API.`,
	Args:  cobra.NoArgs,
	RunE:  runSpec,
}

func runSpec(cmd *cobra.Command, args []string) error {
	return api.GetAndPrint(api.BaseURL + "/spec.json")
}
