package tx

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"strconv"

	"github.com/aptos-labs/aptos-go-sdk"
	"github.com/spf13/cobra"
)

type CallTrace struct {
	From         string            `json:"from"`
	To           string            `json:"to"`
	ContractName string            `json:"contractName"`
	FunctionName string            `json:"functionName"`
	Inputs       []json.RawMessage `json:"inputs"`
	ReturnValue  []json.RawMessage `json:"returnValue"`
	TypeArgs     []string          `json:"typeArgs"`
	GasUsed      uint64            `json:"gasUsed"`
	Calls        []CallTrace       `json:"calls"`
}

var traceCmd = &cobra.Command{
	Use:   "trace <version_or_hash>",
	Short: "Show call trace for a transaction",
	Long:  `Fetches and displays the call trace for an Aptos transaction from Sentio.`,
	Args:  cobra.ExactArgs(1),
	RunE:  runTrace,
}

func runTrace(cmd *cobra.Command, args []string) error {
	hash := args[0]

	// If version, fetch transaction to get hash
	if version, err := strconv.ParseUint(args[0], 10, 64); err == nil {
		client, err := aptos.NewClient(aptos.MainnetConfig)
		if err != nil {
			return fmt.Errorf("failed to create client: %w", err)
		}
		tx, err := client.TransactionByVersion(version)
		if err != nil {
			return fmt.Errorf("failed to fetch transaction %d: %w", version, err)
		}
		hash = string(tx.Hash())
	}

	url := fmt.Sprintf("https://app.sentio.xyz/api/v1/move/call_trace?networkId=1&txHash=%s", hash)

	resp, err := http.Get(url)
	if err != nil {
		return fmt.Errorf("failed to fetch trace: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return fmt.Errorf("API error (status %d): %s", resp.StatusCode, string(body))
	}

	var trace CallTrace
	if err := json.NewDecoder(resp.Body).Decode(&trace); err != nil {
		return fmt.Errorf("failed to decode response: %w", err)
	}

	encoder := json.NewEncoder(os.Stdout)
	encoder.SetIndent("", "  ")
	if err := encoder.Encode(trace); err != nil {
		return fmt.Errorf("failed to encode response: %w", err)
	}

	return nil
}
