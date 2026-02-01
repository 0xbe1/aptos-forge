package tx

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"strconv"

	"github.com/aptos-labs/aptos-go-sdk"
	"github.com/aptos-labs/aptos-go-sdk/api"
)

// isStdinPipe returns true if stdin has piped data
func isStdinPipe() bool {
	stat, err := os.Stdin.Stat()
	if err != nil {
		return false
	}
	return (stat.Mode() & os.ModeCharDevice) == 0
}

// readTransactionFromStdin reads and unmarshals UserTransaction from stdin.
func readTransactionFromStdin() (*api.UserTransaction, error) {
	data, err := io.ReadAll(os.Stdin)
	if err != nil {
		return nil, fmt.Errorf("failed to read from stdin: %w", err)
	}

	var userTx api.UserTransaction
	if err := json.Unmarshal(data, &userTx); err != nil {
		return nil, fmt.Errorf("failed to parse transaction JSON: %w", err)
	}

	return &userTx, nil
}

// getTransaction returns a transaction from stdin or fetches it from the API.
// Returns the transaction, its version (0 for simulated transactions), and any error.
func getTransaction(client *aptos.Client, args []string) (*api.UserTransaction, uint64, error) {
	if isStdinPipe() {
		userTx, err := readTransactionFromStdin()
		if err != nil {
			return nil, 0, err
		}
		// For simulated transactions, Hash is empty and Version is 0
		// For committed transactions piped from `apt tx <version>`, Version is set
		return userTx, userTx.Version, nil
	}

	if len(args) == 0 {
		return nil, 0, fmt.Errorf("no transaction provided")
	}

	return fetchTransaction(client, args[0])
}

// fetchTransaction fetches a transaction by version or hash.
// If the argument parses as a number, it's treated as a version; otherwise as a hash.
// Returns the UserTransaction and its version.
func fetchTransaction(client *aptos.Client, versionOrHash string) (*api.UserTransaction, uint64, error) {
	// Try parsing as version first
	if version, err := strconv.ParseUint(versionOrHash, 10, 64); err == nil {
		tx, err := client.TransactionByVersion(version)
		if err != nil {
			return nil, 0, fmt.Errorf("failed to fetch transaction %d: %w", version, err)
		}
		userTx, err := tx.UserTransaction()
		if err != nil {
			return nil, 0, fmt.Errorf("not a user transaction: %w", err)
		}
		return userTx, version, nil
	}

	// Otherwise treat as hash
	tx, err := client.TransactionByHash(versionOrHash)
	if err != nil {
		return nil, 0, fmt.Errorf("failed to fetch transaction %s: %w", versionOrHash, err)
	}
	userTx, err := tx.UserTransaction()
	if err != nil {
		return nil, 0, fmt.Errorf("not a user transaction: %w", err)
	}
	version := tx.Version()
	if version == nil {
		return nil, 0, fmt.Errorf("transaction not committed yet")
	}
	return userTx, *version, nil
}

// getString extracts a string from a map by traversing nested keys.
// Example: getString(m, "data", "metadata", "inner") returns m["data"]["metadata"]["inner"]
func getString(m map[string]any, keys ...string) string {
	if m == nil || len(keys) == 0 {
		return ""
	}

	current := m
	for _, key := range keys[:len(keys)-1] {
		nested, ok := current[key].(map[string]any)
		if !ok {
			return ""
		}
		current = nested
	}

	if v, ok := current[keys[len(keys)-1]].(string); ok {
		return v
	}
	return ""
}
