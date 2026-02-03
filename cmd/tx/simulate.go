package tx

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/0xbe1/aptly/pkg/api"
	"github.com/aptos-labs/aptos-go-sdk"
	"github.com/aptos-labs/aptos-go-sdk/bcs"
	"github.com/aptos-labs/aptos-go-sdk/crypto"
	"github.com/spf13/cobra"
)

type PayloadInput struct {
	Function      string   `json:"function"`
	TypeArguments []string `json:"type_arguments"`
	Arguments     []any    `json:"arguments"`
}

var simulateCmd = &cobra.Command{
	Use:   "simulate <sender>",
	Short: "Simulate a transaction",
	Long: `Simulate a Move entry function call without a private key.

Reads a JSON payload from stdin with the format:
{
  "function": "0x1::aptos_account::transfer",
  "type_arguments": [],
  "arguments": ["0x2", "1000000"]
}

Examples:
  # Simulate from a payload file
  cat payload.json | apt tx simulate 0x1

  # Replay an existing transaction with a different sender
  apt tx 2658869495 | jq ".payload" | apt tx simulate 0x1`,
	Args: cobra.ExactArgs(1),
	RunE: runSimulate,
}

func runSimulate(cmd *cobra.Command, args []string) error {
	// Read payload from stdin
	data, err := io.ReadAll(os.Stdin)
	if err != nil {
		return fmt.Errorf("failed to read payload from stdin: %w", err)
	}

	var payload PayloadInput
	if err := json.Unmarshal(data, &payload); err != nil {
		return fmt.Errorf("failed to parse payload JSON: %w", err)
	}

	// Parse function identifier (address::module::function)
	funcParts := strings.Split(payload.Function, "::")
	if len(funcParts) != 3 {
		return fmt.Errorf("invalid function format, expected <address>::<module>::<function>")
	}

	moduleAddr := aptos.AccountAddress{}
	if err := moduleAddr.ParseStringRelaxed(funcParts[0]); err != nil {
		return fmt.Errorf("invalid module address: %w", err)
	}
	moduleName := funcParts[1]
	functionName := funcParts[2]

	// Convert type args
	var typeArgs []any
	for _, ta := range payload.TypeArguments {
		typeArgs = append(typeArgs, ta)
	}

	client, err := aptos.NewClient(api.GetNetworkConfig())
	if err != nil {
		return fmt.Errorf("failed to create client: %w", err)
	}

	// Parse sender address
	senderAddr := aptos.AccountAddress{}
	if err := senderAddr.ParseStringRelaxed(args[0]); err != nil {
		return fmt.Errorf("invalid sender address: %w", err)
	}

	// Build entry function using ABI from chain
	entryFunc, err := client.EntryFunctionWithArgs(moduleAddr, moduleName, functionName, typeArgs, payload.Arguments)
	if err != nil {
		return fmt.Errorf("failed to build entry function: %w", err)
	}

	// Build raw transaction
	rawTxn, err := client.BuildTransaction(senderAddr, aptos.TransactionPayload{Payload: entryFunc})
	if err != nil {
		return fmt.Errorf("failed to build transaction: %w", err)
	}

	// Create signed transaction with NoAccountAuthenticator
	// We bypass NewTransactionAuthenticator which doesn't handle AccountAuthenticatorNone
	signedTxn := &aptos.SignedTransaction{
		Transaction: rawTxn,
		Authenticator: &aptos.TransactionAuthenticator{
			Variant: aptos.TransactionAuthenticatorSingleSender,
			Auth: &aptos.SingleSenderTransactionAuthenticator{
				Sender: crypto.NoAccountAuthenticator(),
			},
		},
	}

	// Serialize to BCS
	txnBytes, err := bcs.Serialize(signedTxn)
	if err != nil {
		return fmt.Errorf("failed to serialize transaction: %w", err)
	}

	return api.PostBCSAndPrint(api.BaseURL+"/transactions/simulate", txnBytes)
}
