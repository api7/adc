/*
Copyright © 2023 API7.ai
*/
package cmd

import (
	"github.com/fatih/color"
	"github.com/spf13/cobra"
)

// newDumpCmd represents the dump command
func newDumpCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "dump",
		Short: "Dump the configurations of API7",
		Long:  `The dump command can be used to dump the configurations to the API7.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			color.Green("Scucessfully dump configurations")
			return nil
		},
	}

	cmd.Flags().StringP("output", "o", "", "output file path")

	return cmd
}
