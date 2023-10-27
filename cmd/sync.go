/*
Copyright © 2023 API7.ai
*/
package cmd

import (
	"fmt"
	"strings"
	"time"

	"github.com/fatih/color"
	"github.com/spf13/cobra"

	"github.com/api7/adc/internal/pkg/differ"
	"github.com/api7/adc/pkg/common"
	"github.com/api7/adc/pkg/data"
)

// newSyncCmd represents the configure command
func newSyncCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "sync",
		Short: "Sync local configuration to APISIX",
		Long:  `Syncs the configuration in apisix.yaml (or other provided file) to APISIX.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			checkConfig()

			// TODO: add validate before sync
			_ = sync(cmd, false)
			return nil
		},
	}

	cmd.Flags().StringP("file", "f", "apisix.yaml", "configuration file path")

	return cmd
}

func sync(cmd *cobra.Command, dryRun bool) error {
	file, err := cmd.Flags().GetString("file")
	if err != nil {
		color.Red("Failed to get the configuration file: %v", err)
		return err
	}

	config, err := common.GetContentFromFile(file)
	if err != nil {
		color.Red("Failed to read configuration file: %v", err)
		return err
	}

	if len(config.StreamRoutes) > 0 {
		supportStreamRoute, err := rootConfig.APISIXCluster.SupportStreamRoute()
		if err != nil {
			color.Red("Failed to check stream mode: %v", err)
			return err
		}
		if !supportStreamRoute {
			color.Yellow("Backend stream mode is disabled but configuration contains stream routes, abort")
			return nil
		}
	}

	remoteConfig, err := common.GetContentFromRemote(rootConfig.APISIXCluster)
	if err != nil {
		color.Red("Failed to get remote configuration: %v", err)
		return err
	}

	d, err := differ.NewDiffer(config, remoteConfig)
	if err != nil {
		color.Red("Failed to create a Differ object: %v", err)
		return err
	}

	events, err := d.Diff()
	if err != nil {
		color.Red("Failed to compare local and remote configuration: %v", err)
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

		str, err := event.Output(dryRun)
		if err != nil {
			color.Red("Failed to get output of the event: %v", err)
			return err
		}

		if !dryRun {
			err = event.Apply(rootConfig.APISIXCluster)
			if err != nil {
				color.Red("Failed to apply configuration: %v", err)
				return err
			}
			time.Sleep(100 * time.Millisecond)
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

	if dryRun {
		color.Green("Summary: create %d, update %d, delete %d", summary.created, summary.updated, summary.deleted)
	} else {
		color.Green("Summary: created %d, updated %d, deleted %d", summary.created, summary.updated, summary.deleted)
	}

	return nil
}
