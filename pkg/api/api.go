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

// GetJSON makes a GET request and returns the parsed JSON response.
func GetJSON(url string) (any, error) {
	resp, err := http.Get(url)
	if err != nil {
		return nil, fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read response: %w", err)
	}

	if resp.StatusCode != http.StatusOK && resp.StatusCode != http.StatusAccepted {
		return nil, fmt.Errorf("API error (status %d): %s", resp.StatusCode, string(body))
	}

	var data any
	if err := json.Unmarshal(body, &data); err != nil {
		return nil, fmt.Errorf("failed to parse response: %w", err)
	}

	return data, nil
}

// PrintJSON pretty-prints data as JSON to stdout.
func PrintJSON(data any) error {
	encoder := json.NewEncoder(os.Stdout)
	encoder.SetIndent("", "  ")
	return encoder.Encode(data)
}

// GetAndPrint makes a GET request to the URL and prints the JSON response.
func GetAndPrint(url string) error {
	data, err := GetJSON(url)
	if err != nil {
		return err
	}
	return PrintJSON(data)
}

// PostAndPrint makes a POST request with JSON body and prints the JSON response.
func PostAndPrint(url string, body any) error {
	bodyBytes, err := json.Marshal(body)
	if err != nil {
		return fmt.Errorf("failed to marshal request body: %w", err)
	}
	return postRawAndPrint(url, bodyBytes, "application/json", false)
}

// PostBCSAndPrint posts BCS-encoded data and prints the first element of the JSON array response.
func PostBCSAndPrint(url string, body []byte) error {
	return postRawAndPrint(url, body, "application/x.aptos.signed_transaction+bcs", true)
}

// postRawAndPrint posts raw bytes with given content type and prints the response.
func postRawAndPrint(url string, body []byte, contentType string, extractFirst bool) error {
	resp, err := http.Post(url, contentType, bytes.NewReader(body))
	if err != nil {
		return fmt.Errorf("request failed: %w", err)
	}
	defer resp.Body.Close()

	return handleResponse(resp, extractFirst)
}

// handleResponse reads the response body, checks status, and pretty-prints JSON.
// If extractFirst is true, extracts the first element from an array response.
func handleResponse(resp *http.Response, extractFirst bool) error {
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

	if extractFirst {
		arr, ok := data.([]any)
		if !ok {
			return fmt.Errorf("expected array response")
		}
		if len(arr) == 0 {
			return fmt.Errorf("no result returned")
		}
		data = arr[0]
	}

	return PrintJSON(data)
}
