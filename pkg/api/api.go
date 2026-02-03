package api

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"

	aptos "github.com/aptos-labs/aptos-go-sdk"
)

var BaseURL = "https://api.mainnet.aptoslabs.com/v1"

// GetNetworkConfig returns an Aptos NetworkConfig using the current BaseURL.
func GetNetworkConfig() aptos.NetworkConfig {
	return aptos.NetworkConfig{
		NodeUrl: BaseURL,
	}
}

// GetAndPrint makes a GET request to the URL and prints the JSON response.
func GetAndPrint(url string) error {
	resp, err := http.Get(url)
	if err != nil {
		return fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	return handleResponse(resp)
}

// PostAndPrint makes a POST request with JSON body and prints the JSON response.
func PostAndPrint(url string, body any) error {
	bodyBytes, err := json.Marshal(body)
	if err != nil {
		return fmt.Errorf("failed to marshal request body: %w", err)
	}

	resp, err := http.Post(url, "application/json", bytes.NewReader(bodyBytes))
	if err != nil {
		return fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	return handleResponse(resp)
}

// PostBCSAndPrint posts BCS-encoded data and prints the first element of the JSON array response.
func PostBCSAndPrint(url string, body []byte) error {
	resp, err := http.Post(url, "application/x.aptos.signed_transaction+bcs", bytes.NewReader(body))
	if err != nil {
		return fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		return fmt.Errorf("failed to read response: %w", err)
	}

	if resp.StatusCode != http.StatusOK && resp.StatusCode != http.StatusAccepted {
		return fmt.Errorf("API error (status %d): %s", resp.StatusCode, string(respBody))
	}

	var result []any
	if err := json.Unmarshal(respBody, &result); err != nil {
		return fmt.Errorf("failed to parse response: %w", err)
	}

	if len(result) == 0 {
		return fmt.Errorf("no result returned")
	}

	encoder := json.NewEncoder(os.Stdout)
	encoder.SetIndent("", "  ")
	return encoder.Encode(result[0])
}

// handleResponse reads the response body, checks status, and pretty-prints JSON.
func handleResponse(resp *http.Response) error {
	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return fmt.Errorf("failed to read response: %w", err)
	}

	if resp.StatusCode != http.StatusOK && resp.StatusCode != http.StatusAccepted {
		return fmt.Errorf("API error (status %d): %s", resp.StatusCode, string(body))
	}

	var data any
	if err := json.Unmarshal(body, &data); err != nil {
		return fmt.Errorf("failed to parse response: %w", err)
	}

	encoder := json.NewEncoder(os.Stdout)
	encoder.SetIndent("", "  ")
	return encoder.Encode(data)
}
