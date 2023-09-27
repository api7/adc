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
		Short: "Show the differences between the local and existing APISIX configuration",
		Long:  `Shows the differences in the configuration between the local confguration file and the connected APISIX instance.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			checkConfig()

			_ = sync(cmd, true)
			return nil
		},
	}

	cmd.Flags().StringP("file", "f", "apisix.yaml", "configuration file path")
	return cmd
}
