package events

import (
	"fmt"

	"github.com/0xbe1/aptly/pkg/api"
	"github.com/spf13/cobra"
)

var (
	eventsLimit uint64
	eventsStart uint64
)

var EventsCmd = &cobra.Command{
	Use:   "events <address> <creation_number>",
	Short: "Get events by creation number",
	Long: `Fetches and displays events for an account by creation number from the Aptos mainnet.

Examples:
  apt events 0x1 0
  apt events 0x1 0 --limit 10
  apt events 0x1 0 --limit 10 --start 100`,
	Args: cobra.ExactArgs(2),
	RunE: runEvents,
}

func init() {
	EventsCmd.Flags().Uint64Var(&eventsLimit, "limit", 25, "Maximum number of events to return")
	EventsCmd.Flags().Uint64Var(&eventsStart, "start", 0, "Starting sequence number (0 means start from the beginning)")
}

func runEvents(cmd *cobra.Command, args []string) error {
	url := fmt.Sprintf("%s/accounts/%s/events/%s?limit=%d", api.BaseURL, args[0], args[1], eventsLimit)
	if eventsStart > 0 {
		url = fmt.Sprintf("%s&start=%d", url, eventsStart)
	}
	return api.GetAndPrint(url)
}
