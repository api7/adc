package cmd

import (
	"github.com/fatih/color"
	"github.com/spf13/cobra"
)

var (
	VERSION     = "dev"
	GitRevision = "unknown"
)

// newVersionCmd represents the version command
func newVersionCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "version",
		Short: "Print the version of adc",
		Long:  `The version command can be used to print the version of adc.`,
		Run: func(cmd *cobra.Command, args []string) {
			color.Green("adc version: %s - %s\n", VERSION, GitRevision)
		},
	}

	return cmd
}
