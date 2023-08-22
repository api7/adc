/*
Copyright © 2023 API7.ai
*/
package cmd

import (
	"fmt"
	"strings"

	"github.com/fatih/color"
	"github.com/spf13/cobra"

	"github.com/api7/adc/internal/pkg/differ"
	"github.com/api7/adc/pkg/common"
	"github.com/api7/adc/pkg/data"
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
