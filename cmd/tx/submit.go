package tx

import (
	"encoding/json"
	"fmt"
	"io"
	"os"

	"github.com/0xbe1/aptly/pkg/api"
	"github.com/spf13/cobra"
)

var submitCmd = &cobra.Command{
	Use:   "submit",
	Short: "Submit a signed transaction",
	Long: `Submits a signed transaction to the Aptos mainnet.

Reads a signed transaction from stdin in JSON format.

Example workflow:
  1. Build and encode: echo '{"sender":"0x1",...}' | apt tx encode
  2. Sign the output externally (wallet, hardware key, etc.)
  3. Submit: echo '{"signature":"...",...}' | apt tx submit

Example:
  cat signed_tx.json | apt tx submit`,
	Args: cobra.NoArgs,
	RunE: runSubmit,
}

func init() {
	TxCmd.AddCommand(submitCmd)
}

func runSubmit(cmd *cobra.Command, args []string) error {
	data, err := io.ReadAll(os.Stdin)
	if err != nil {
		return fmt.Errorf("failed to read signed transaction from stdin: %w", err)
	}

	var txn any
	if err := json.Unmarshal(data, &txn); err != nil {
		return fmt.Errorf("failed to parse signed transaction JSON: %w", err)
	}

	return api.PostAndPrint(api.BaseURL+"/transactions", txn)
}
