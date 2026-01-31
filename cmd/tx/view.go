package tx

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/aptos-labs/aptos-go-sdk"
	"github.com/spf13/cobra"
)

var viewCmd = &cobra.Command{
	Use:   "view <version_or_hash>",
	Short: "View a transaction by version or hash",
	Long:  `Fetches and displays an Aptos transaction in JSON format.`,
	Args:  cobra.ExactArgs(1),
	RunE:  runView,
}

func runView(cmd *cobra.Command, args []string) error {
	client, err := aptos.NewClient(aptos.MainnetConfig)
	if err != nil {
		return fmt.Errorf("failed to create client: %w", err)
	}

	userTx, _, err := fetchTransaction(client, args[0])
	if err != nil {
		return err
	}

	encoder := json.NewEncoder(os.Stdout)
	encoder.SetIndent("", "  ")
	if err := encoder.Encode(userTx); err != nil {
		return fmt.Errorf("failed to encode response: %w", err)
	}

	return nil
}
