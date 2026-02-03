package account

import (
	"fmt"

	"github.com/0xbe1/aptly/pkg/api"
	"github.com/spf13/cobra"
)

var (
	moduleABI      bool
	moduleBytecode bool
)

var moduleCmd = &cobra.Command{
	Use:   "module <address> <module_name>",
	Short: "Get a specific account module",
	Long: `Fetches and displays a specific module for an account from the Aptos mainnet.

Examples:
  aptly account module 0x1 coin
  aptly account module 0x1 coin --abi
  aptly account module 0x1 coin --bytecode`,
	Args: cobra.ExactArgs(2),
	RunE: runModule,
}

func init() {
	moduleCmd.Flags().BoolVar(&moduleABI, "abi", false, "Output only the ABI")
	moduleCmd.Flags().BoolVar(&moduleBytecode, "bytecode", false, "Output only the bytecode")
}

func runModule(cmd *cobra.Command, args []string) error {
	url := fmt.Sprintf("%s/accounts/%s/module/%s", api.BaseURL, args[0], args[1])

	// If no filter flags, print everything
	if !moduleABI && !moduleBytecode {
		return api.GetAndPrint(url)
	}

	// Fetch and extract the requested field
	data, err := api.GetJSON(url)
	if err != nil {
		return err
	}

	module := data.(map[string]any)
	if moduleABI {
		return api.PrintJSON(module["abi"])
	}
	return api.PrintJSON(module["bytecode"])
}
