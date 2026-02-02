package ledger

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"

	"github.com/spf13/cobra"
)

var LedgerCmd = &cobra.Command{
	Use:   "ledger",
	Short: "Get ledger information",
	Long:  `Fetches and displays the current ledger information from the Aptos mainnet.`,
	Args:  cobra.NoArgs,
	RunE:  runLedger,
}

func runLedger(cmd *cobra.Command, args []string) error {
	url := "https://api.mainnet.aptoslabs.com/v1/"

	resp, err := http.Get(url)
	if err != nil {
		return fmt.Errorf("failed to fetch ledger info: %w", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return fmt.Errorf("failed to read response: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("API error (status %d): %s", resp.StatusCode, string(body))
	}

	var data any
	if err := json.Unmarshal(body, &data); err != nil {
		return fmt.Errorf("failed to parse response: %w", err)
	}

	encoder := json.NewEncoder(os.Stdout)
	encoder.SetIndent("", "  ")
	if err := encoder.Encode(data); err != nil {
		return fmt.Errorf("failed to encode response: %w", err)
	}

	return nil
}
