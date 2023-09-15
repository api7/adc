/*
Copyright © 2023 API7.ai
*/
package cmd

import (
	"context"
	"fmt"

	"github.com/fatih/color"
	"github.com/spf13/cobra"
	"sigs.k8s.io/yaml"

	"github.com/api7/adc/pkg/api/apisix"
	"github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/pkg/common"
)

// newDumpCmd represents the dump command
func newDumpCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "dump",
		Short: "Dump the APISIX configuration",
		Long:  `Dumps the configuration of the connected APISIX instance to a local file.`,
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
		color.Red("Failed to get output file path: %v", err)
		return err
	}
	if path == "" {
		color.Red("Output file path is empty. Please specify a file path: adc dump -o config.yaml")
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

	consumers, err := cluster.Consumer().List(context.Background())
	if err != nil {
		return err
	}

	ssls, err := cluster.SSL().List(context.Background())
	if err != nil {
		return err
	}

	globalRules, err := cluster.GlobalRule().List(context.Background())
	if err != nil {
		return err
	}

	pluginConfigs, err := cluster.PluginConfig().List(context.Background())
	if err != nil {
		return err
	}

	consumerGroups, err := cluster.ConsumerGroup().List(context.Background())
	if err != nil {
		return err
	}

	pluginMetadatas, err := cluster.PluginMetadata().List(context.Background())
	if err != nil {
		return err
	}

	conf := &types.Configuration{
		Routes:          routes,
		Services:        svcs,
		Consumers:       consumers,
		SSLs:            ssls,
		GlobalRules:     globalRules,
		PluginConfigs:   pluginConfigs,
		ConsumerGroups:  consumerGroups,
		PluginMetadatas: pluginMetadatas,
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
