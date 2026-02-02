package cmd

import (
	"os"

	"github.com/0xbe1/apt/cmd/ledger"
	"github.com/0xbe1/apt/cmd/tx"
	"github.com/spf13/cobra"
)

var rootCmd = &cobra.Command{
	Use:   "apt",
	Short: "Aptos CLI utilities for agents",
	Long:  `A collection of Aptos utilities designed for easy integration with AI agents.`,
}

func Execute() {
	err := rootCmd.Execute()
	if err != nil {
		os.Exit(1)
	}
}

func init() {
	rootCmd.AddCommand(ledger.LedgerCmd)
	rootCmd.AddCommand(tx.TxCmd)
}
