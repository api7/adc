/*
Copyright © 2023 API7.ai
*/
package cmd

import (
	"github.com/api7/adc/internal/pkg/differ"
	"github.com/api7/adc/pkg/common"
	"github.com/fatih/color"

	"github.com/spf13/cobra"
)

// newDiffCmd represents the diff command
func newDiffCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "diff",
		Short: "Diff the configurations between local and API7",
		Long:  `The diff command can be used to diff the configurations between local and API7.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			return diffAPI7(cmd)
		},
	}

	cmd.Flags().StringP("file", "f", "adc.yaml", "configuration file path")
	return cmd
}

func diffAPI7(cmd *cobra.Command) error {
	file, err := cmd.Flags().GetString("file")
	if err != nil {
		color.Red("Get file path failed: %v", err)
		return err
	}

	config, err := common.GetContentFromFile(file)
	if err != nil {
		color.Red("Get file content failed: %v", err)
		return err
	}

	remoteConifg, err := common.GetContentFromRemote()
	if err != nil {
		color.Red("Failed to get remote config: %v", err)
		return err
	}

	d, err := differ.NewDiffer(config, remoteConifg)
	if err != nil {
		color.Red("Failed to create differ: %v", err)
		return err
	}

	return d.Diff()
}
