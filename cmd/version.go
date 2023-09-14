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
		Short: "Print the version of ADC",
		Long:  `Prints the version of ADC. See https://github.com/api7/adc for details on how to update.`,
		Run: func(cmd *cobra.Command, args []string) {
			color.Green("ADC version: %s - %s\n", VERSION, GitRevision)
		},
	}

	return cmd
}
