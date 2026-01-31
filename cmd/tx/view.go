package tx

import (
	"encoding/json"
	"fmt"
	"os"
	"strconv"

	"github.com/aptos-labs/aptos-go-sdk"
	"github.com/spf13/cobra"
)

var viewCmd = &cobra.Command{
	Use:   "view <tx_version>",
	Short: "View a transaction by version",
	Long:  `Fetches and displays an Aptos transaction by its version number in JSON format.`,
	Args:  cobra.ExactArgs(1),
	RunE:  runView,
}

func runView(cmd *cobra.Command, args []string) error {
	version, err := strconv.ParseUint(args[0], 10, 64)
	if err != nil {
		return fmt.Errorf("invalid transaction version: %w", err)
	}

	client, err := aptos.NewClient(aptos.MainnetConfig)
	if err != nil {
		return fmt.Errorf("failed to create client: %w", err)
	}

	tx, err := client.TransactionByVersion(version)
	if err != nil {
		return fmt.Errorf("failed to fetch transaction: %w", err)
	}

	encoder := json.NewEncoder(os.Stdout)
	encoder.SetIndent("", "  ")
	if err := encoder.Encode(tx); err != nil {
		return fmt.Errorf("failed to encode response: %w", err)
	}

	return nil
}
