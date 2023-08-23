/*
Copyright © 2023 API7.ai
*/
package cmd

import (
	"github.com/spf13/cobra"
)

// newDiffCmd represents the diff command
func newDiffCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "diff",
		Short: "Diff the configurations between local and API7",
		Long:  `The diff command can be used to diff the configurations between local and API7.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			_ = sync(cmd, true)
			return nil
		},
	}

	cmd.Flags().StringP("file", "f", "adc.yaml", "configuration file path")
	return cmd
}
