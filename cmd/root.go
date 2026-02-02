package cmd

import (
	"os"

	"github.com/0xbe1/apt/cmd/account"
	"github.com/0xbe1/apt/cmd/block"
	"github.com/0xbe1/apt/cmd/events"
	"github.com/0xbe1/apt/cmd/node"
	"github.com/0xbe1/apt/cmd/table"
	"github.com/0xbe1/apt/cmd/tx"
	"github.com/0xbe1/apt/cmd/view"
	"github.com/0xbe1/apt/pkg/api"
	"github.com/spf13/cobra"
)

var rpcURL string

var rootCmd = &cobra.Command{
	Use:   "apt",
	Short: "Aptos CLI utilities for agents",
	Long:  `A collection of Aptos utilities designed for easy integration with AI agents.`,
	PersistentPreRun: func(cmd *cobra.Command, args []string) {
		if rpcURL != "" {
			api.BaseURL = rpcURL
		}
	},
}

func Execute() {
	err := rootCmd.Execute()
	if err != nil {
		os.Exit(1)
	}
}

func init() {
	rootCmd.PersistentFlags().StringVar(&rpcURL, "rpc-url", "", "Custom RPC URL (default: mainnet)")

	rootCmd.AddCommand(account.AccountCmd)
	rootCmd.AddCommand(block.BlockCmd)
	rootCmd.AddCommand(events.EventsCmd)
	rootCmd.AddCommand(node.NodeCmd)
	rootCmd.AddCommand(table.TableCmd)
	rootCmd.AddCommand(tx.TxCmd)
	rootCmd.AddCommand(view.ViewCmd)
}
