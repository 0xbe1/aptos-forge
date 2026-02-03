package account

import (
	"bytes"
	"compress/gzip"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/0xbe1/aptly/pkg/api"
	"github.com/aptos-labs/aptos-go-sdk"
	"github.com/spf13/cobra"
)

var (
	sourceCodePackage string
	sourceCodeRaw     bool
)

var sourceCodeCmd = &cobra.Command{
	Use:   "source-code <address> [module_name]",
	Short: "Extract Move source code from on-chain PackageMetadata",
	Long: `Extracts and displays Move source code stored in on-chain PackageMetadata.

Source code is only available if the package was published with the --save-metadata flag.

Examples:
  aptly account source-code 0x1              # All source from all packages
  aptly account source-code 0x1 coin         # Specific module
  aptly account source-code 0x1 coin --raw   # Raw output for piping
  aptly account source-code 0x1 --package AptosFramework  # Filter by package`,
	Args: cobra.RangeArgs(1, 2),
	RunE: runSourceCode,
}

func init() {
	sourceCodeCmd.Flags().StringVar(&sourceCodePackage, "package", "", "Filter by package name")
	sourceCodeCmd.Flags().BoolVar(&sourceCodeRaw, "raw", false, "Output raw source without JSON wrapper")
}

// ModuleSource represents the source code of a module
type ModuleSource struct {
	Package string `json:"package"`
	Module  string `json:"module"`
	Source  string `json:"source"`
}

// PackageRegistry structure from 0x1::code::PackageRegistry
type PackageRegistry struct {
	Packages []PackageMetadata `json:"packages"`
}

// PackageMetadata structure from 0x1::code
type PackageMetadata struct {
	Name    string           `json:"name"`
	Modules []ModuleMetadata `json:"modules"`
}

// ModuleMetadata structure from 0x1::code
type ModuleMetadata struct {
	Name   string `json:"name"`
	Source string `json:"source"` // hex-encoded gzipped source
}

func runSourceCode(cmd *cobra.Command, args []string) error {
	client, err := aptos.NewClient(api.GetNetworkConfig())
	if err != nil {
		return fmt.Errorf("failed to create client: %w", err)
	}

	var addr aptos.AccountAddress
	if err := addr.ParseStringRelaxed(args[0]); err != nil {
		return fmt.Errorf("invalid address: %w", err)
	}

	var moduleName string
	if len(args) > 1 {
		moduleName = args[1]
	}

	// Fetch PackageRegistry resource using SDK
	resourceData, err := client.AccountResource(addr, "0x1::code::PackageRegistry")
	if err != nil {
		if strings.Contains(err.Error(), "resource_not_found") {
			return fmt.Errorf("no code found at address")
		}
		return fmt.Errorf("failed to fetch resource: %w", err)
	}

	// Parse the resource data into PackageRegistry
	// The SDK returns {"data": {"packages": [...]}} so we need to extract the inner data
	dataBytes, err := json.Marshal(resourceData)
	if err != nil {
		return fmt.Errorf("failed to marshal resource data: %w", err)
	}

	var wrapper struct {
		Data PackageRegistry `json:"data"`
	}
	if err := json.Unmarshal(dataBytes, &wrapper); err != nil {
		return fmt.Errorf("failed to parse resource: %w", err)
	}
	registry := wrapper.Data

	// Extract source code from all matching modules
	var sources []ModuleSource
	for _, pkg := range registry.Packages {
		// Filter by package name if specified
		if sourceCodePackage != "" && pkg.Name != sourceCodePackage {
			continue
		}

		for _, mod := range pkg.Modules {
			// Filter by module name if specified
			if moduleName != "" && mod.Name != moduleName {
				continue
			}

			// Skip if no source available
			if mod.Source == "" {
				continue
			}

			// Decode hex and decompress gzip
			source, err := decodeSource(mod.Source)
			if err != nil {
				// Skip modules with undecodable source
				continue
			}

			sources = append(sources, ModuleSource{
				Package: pkg.Name,
				Module:  mod.Name,
				Source:  source,
			})
		}
	}

	// Check if we found any sources
	if len(sources) == 0 {
		if moduleName != "" {
			// Check if the module exists but has no source
			moduleExists := false
			for _, pkg := range registry.Packages {
				if sourceCodePackage != "" && pkg.Name != sourceCodePackage {
					continue
				}
				for _, mod := range pkg.Modules {
					if mod.Name == moduleName {
						moduleExists = true
						break
					}
				}
			}
			if moduleExists {
				return fmt.Errorf("no source code available (compiled without --save-metadata)")
			}
			return fmt.Errorf("module %q not found", moduleName)
		}
		return fmt.Errorf("no source code available (compiled without --save-metadata)")
	}

	// Output
	if sourceCodeRaw {
		if len(sources) > 1 {
			return fmt.Errorf("--raw requires exactly one module match (found %d)", len(sources))
		}
		fmt.Print(sources[0].Source)
		return nil
	}

	encoder := json.NewEncoder(os.Stdout)
	encoder.SetIndent("", "  ")
	return encoder.Encode(sources)
}

// decodeSource decodes hex-encoded gzipped source code
func decodeSource(hexSource string) (string, error) {
	// Remove "0x" prefix if present
	if len(hexSource) >= 2 && hexSource[:2] == "0x" {
		hexSource = hexSource[2:]
	}

	// Decode hex
	gzipped, err := hex.DecodeString(hexSource)
	if err != nil {
		return "", fmt.Errorf("failed to decode hex: %w", err)
	}

	// Decompress gzip
	reader, err := gzip.NewReader(bytes.NewReader(gzipped))
	if err != nil {
		return "", fmt.Errorf("failed to create gzip reader: %w", err)
	}
	defer reader.Close()

	source, err := io.ReadAll(reader)
	if err != nil {
		return "", fmt.Errorf("failed to decompress: %w", err)
	}

	return string(source), nil
}
