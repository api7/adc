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
	"github.com/api7/adc/pkg/api/apisix/types"
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
			err := sync(cmd, false)
			return err
		},
	}

	cmd.Flags().StringArrayP("file", "f", []string{"apisix.yaml"}, "configuration file path")
	cmd.Flags().BoolP("partial", "p", false, "partial apply mode. In partial mode, only add and update event will be applied.")
	cmd.Flags().BoolP("dryRun", "d", false, "dry run mode. In dry run mode, no configuration will be applied.")

	return cmd
}

type summary struct {
	created int
	updated int
	deleted int
}

func syncFile(dryRun, partial bool, file string) (*summary, error) {
	config, err := common.GetContentFromFile(file)
	if err != nil {
		color.Red("Failed to read configuration file: %v", err)
		return nil, err
	}
	if config.Meta != nil {
		if config.Meta.Mode == types.ModePartial {
			partial = true
		}
	}

	if len(config.StreamRoutes) > 0 {
		supportStreamRoute, err := rootConfig.APISIXCluster.SupportStreamRoute()
		if err != nil {
			color.Red("Failed to check stream mode: %v", err)
			return nil, err
		}
		if !supportStreamRoute {
			color.Yellow("Backend stream mode is disabled but configuration contains stream routes, abort")
			return &summary{
				created: 0,
				updated: 0,
				deleted: 0,
			}, nil
		}
	}

	remoteConfig, err := common.GetContentFromRemote(rootConfig.APISIXCluster)
	if err != nil {
		color.Red("Failed to get remote configuration: %v", err)
		return nil, err
	}

	d, err := differ.NewDiffer(config, remoteConfig)
	if err != nil {
		color.Red("Failed to create a Differ object: %v", err)
		return nil, err
	}

	events, err := d.Diff()
	if err != nil {
		color.Red("Failed to compare local and remote configuration: %v", err)
		return nil, err
	}

	summary := &summary{
		created: 0,
		updated: 0,
		deleted: 0,
	}

	for _, event := range events {
		if event.Option == data.CreateOption {
			summary.created++
		} else if event.Option == data.UpdateOption {
			summary.updated++
		} else if event.Option == data.DeleteOption {
			if partial {
				continue
			}
			summary.deleted++
		}

		str, err := event.Output(dryRun)
		if err != nil {
			color.Red("Failed to get output of the event: %v", err)
			return nil, err
		}

		if !dryRun {
			err = event.Apply(rootConfig.APISIXCluster)
			if err != nil {
				color.Red("Failed to apply configuration: %v", err)
				return nil, err
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

	return summary, nil
}

func sync(cmd *cobra.Command, dryRun bool) error {
	files, err := cmd.Flags().GetStringArray("file")
	if err != nil {
		color.Red("Failed to get the configuration file: %v", err)
		return err
	}
	if len(files) == 0 {
		color.Red("No input files")
		return nil
	}

	dryRun, err = cmd.Flags().GetBool("dryRun")
	if err != nil {
		color.Red("Failed to get dry run option: %v", err)
		return err
	}

	partial := false

	if !dryRun {
		partial, err = cmd.Flags().GetBool("partial")
		if err != nil {
			color.Red("Failed to get partial option: %v", err)
			return err
		}
	}

	if len(files) > 1 {
		partial = true
	}

	summary := &summary{
		created: 0,
		updated: 0,
		deleted: 0,
	}

	for _, file := range files {
		sum, err := syncFile(dryRun, partial, file)
		if err != nil {
			color.Red("failed to sync file %v, error: %v", file, err)
			continue
		}

		summary.created += sum.created
		summary.updated += sum.updated
		summary.deleted += sum.deleted
	}

	if dryRun {
		color.Green("Summary: create %d, update %d, delete %d", summary.created, summary.updated, summary.deleted)
	} else {
		color.Green("Summary: created %d, updated %d, deleted %d", summary.created, summary.updated, summary.deleted)
	}

	return nil
}
