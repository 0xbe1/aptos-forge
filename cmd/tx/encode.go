package tx

import (
	"encoding/json"
	"fmt"
	"io"
	"os"

	"github.com/0xbe1/aptly/pkg/api"
	"github.com/spf13/cobra"
)

var encodeCmd = &cobra.Command{
	Use:   "encode",
	Short: "Encode a transaction for signing",
	Long: `Encodes a transaction for signing using the Aptos API.

Reads an unsigned transaction from stdin and returns the signing message.
This is useful for external signing workflows where you need to sign
with a hardware wallet or other external signer.

Input format (from stdin):
{
  "sender": "0x...",
  "sequence_number": "0",
  "max_gas_amount": "200000",
  "gas_unit_price": "100",
  "expiration_timestamp_secs": "...",
  "payload": {
    "type": "entry_function_payload",
    "function": "0x1::aptos_account::transfer",
    "type_arguments": [],
    "arguments": ["0x2", "1000000"]
  }
}

Example:
  cat unsigned_tx.json | apt tx encode`,
	Args: cobra.NoArgs,
	RunE: runEncode,
}

func init() {
	TxCmd.AddCommand(encodeCmd)
}

func runEncode(cmd *cobra.Command, args []string) error {
	data, err := io.ReadAll(os.Stdin)
	if err != nil {
		return fmt.Errorf("failed to read transaction from stdin: %w", err)
	}

	var txn any
	if err := json.Unmarshal(data, &txn); err != nil {
		return fmt.Errorf("failed to parse transaction JSON: %w", err)
	}

	return api.PostAndPrint(api.BaseURL+"/transactions/encode_submission", txn)
}
