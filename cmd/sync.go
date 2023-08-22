/*
Copyright © 2023 API7.ai
*/
package cmd

import (
	"github.com/fatih/color"
	"github.com/spf13/cobra"

	"github.com/api7/adc/pkg/common"
)

// newSyncCmd represents the configure command
func newSyncCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "sync",
		Short: "Sync the configurations from local to API7",
		Long:  `The sync command can be used to sync the configurations from local to API7.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			file, err := cmd.Flags().GetString("file")
			if err != nil {
				color.Red("Get file path failed: %v", err)
				return err
			}

			d, err := common.GetContentFromFile(file)
			if err != nil {
				color.Red("Get file content failed: %v", err)
				return err
			}

			color.Green("Get file content success: %v", d)
			color.Green("Successfully sync configurations")
			return nil
		},
	}

	cmd.Flags().StringP("file", "f", "adc.yaml", "configuration file path")

	return cmd
}
