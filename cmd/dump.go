/*
Copyright © 2023 API7.ai
*/
package cmd

import (
	"context"
	"fmt"
	"sigs.k8s.io/yaml"

	"github.com/fatih/color"
	"github.com/spf13/cobra"

	"github.com/api7/adc/pkg/api/apisix"
	"github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/pkg/common"
)

// newDumpCmd represents the dump command
func newDumpCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "dump",
		Short: "Dump the configurations of API7",
		Long:  `The dump command can be used to dump the configurations to the API7.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			checkConfig()

			err := dumpConfiguration(cmd)
			if err != nil {
				color.Red(err.Error())
			}
			return err
		},
	}

	cmd.Flags().StringP("output", "o", "", "output file path")

	return cmd
}

func dumpConfiguration(cmd *cobra.Command) error {
	path, err := cmd.Flags().GetString("output")
	if err != nil {
		color.Red("Get file path failed: %v", err)
		return err
	}
	if path == "" {
		color.Red("Output path is empty. Example: adc dump -o config.yaml")
		return nil
	}

	save := true
	if path == "/dev/stdout" {
		save = false
	}

	cluster := apisix.NewCluster(context.Background(), rootConfig.Server, rootConfig.Token)

	svcs, err := cluster.Service().List(context.Background())
	if err != nil {
		return err
	}

	routes, err := cluster.Route().List(context.Background())
	if err != nil {
		return err
	}

	conf := &types.Configuration{
		Routes:   routes,
		Services: svcs,
	}

	if save {
		err = common.SaveAPISIXConfiguration(path, conf)
		if err != nil {
			return err
		}
		color.Green("Successfully dump configurations to " + path)
	} else {
		data, err := yaml.Marshal(conf)
		if err != nil {
			color.Red(err.Error())
			return err
		}

		_, err = fmt.Printf(string(data))
		if err != nil {
			return err
		}
	}

	return nil
}
