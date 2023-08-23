/*
Copyright © 2023 API7.ai
*/
package cmd

import (
	"fmt"
	"github.com/api7/adc/internal/pkg/differ"
	"github.com/api7/adc/pkg/data"
	"github.com/fatih/color"
	"github.com/spf13/cobra"
	"strings"

	"github.com/api7/adc/pkg/common"
)

// newSyncCmd represents the configure command
func newSyncCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "sync",
		Short: "Sync the configurations from local to API7",
		Long:  `The sync command can be used to sync the configurations from local to API7.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			// TODO: add validate before sync
			_ = sync(cmd, false)
			return nil
		},
	}

	cmd.Flags().StringP("file", "f", "adc.yaml", "configuration file path")

	return cmd
}

func sync(cmd *cobra.Command, dryRun bool) error {
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

	remoteConifg, err := common.GetContentFromRemote(rootConfig.APISIXCluster)
	if err != nil {
		color.Red("Failed to get remote config: %v", err)
		return err
	}

	d, err := differ.NewDiffer(config, remoteConifg)
	if err != nil {
		color.Red("Failed to create differ: %v", err)
		return err
	}

	events, err := d.Diff()
	if err != nil {
		color.Red("Failed to diff: %v", err)
		return err
	}

	var summary struct {
		created int
		updated int
		deleted int
	}

	for _, event := range events {
		if event.Option == data.CreateOption {
			summary.created++
		} else if event.Option == data.UpdateOption {
			summary.updated++
		} else if event.Option == data.DeleteOption {
			summary.deleted++
		}

		str, err := event.Output()
		if err != nil {
			color.Red("Failed to get output: %v", err)
			return err
		}

		if !dryRun {
			err = event.Apply(rootConfig.APISIXCluster)
			if err != nil {
				color.Red("Failed to apply: %v", err)
				return err
			}
		}

		for _, line := range strings.Split(str, "\n") {
			if strings.HasPrefix(line, "+") || strings.HasPrefix(line, "creating") {
				color.Green(line)
			} else if strings.HasPrefix(line, "-") || strings.HasPrefix(line, "deleting") {
				color.Red(line)
			} else {
				fmt.Println(line)
			}
		}
	}

	color.Green("Summary: created %d, updated %d, deleted %d", summary.created, summary.updated, summary.deleted)

	return nil
}
