package tx

import (
	"fmt"
	"strconv"

	"github.com/aptos-labs/aptos-go-sdk"
	"github.com/aptos-labs/aptos-go-sdk/api"
)

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
