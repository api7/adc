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

	cmd.Flags().StringP("output", "o", "/dev/stdout", "output file path")
	cmd.Flags().StringToStringP("labels", "l", map[string]string{}, "labels to filter resources")

	return cmd
}

func dumpConfiguration(cmd *cobra.Command) error {
	path, err := cmd.Flags().GetString("output")
	if err != nil {
		color.Red("Failed to get output file path: %v", err)
		return err
	}
	if path == "" {
		path = "/dev/stdout"
	}

	save := true
	if path == "/dev/stdout" {
		save = false
	}

	labels, err := cmd.Flags().GetStringToString("labels")
	if err != nil {
		return err
	}

	cluster, err := apisix.NewCluster(context.Background(), rootConfig.ClientConfig)
	if err != nil {
		return err
	}

	svcs, err := cluster.Service().List(context.Background())
	if err != nil {
		return err
	}

	svcs = types.FilterResources(labels, svcs)

	routes, err := cluster.Route().List(context.Background())
	if err != nil {
		return err
	}

	routes = types.FilterResources(labels, routes)

	consumers, err := cluster.Consumer().List(context.Background())
	if err != nil {
		return err
	}

	consumers = types.FilterResources(labels, consumers)

	ssls, err := cluster.SSL().List(context.Background())
	if err != nil {
		return err
	}

	ssls = types.FilterResources(labels, ssls)

	globalRules, err := cluster.GlobalRule().List(context.Background())
	if err != nil {
		return err
	}

	pluginConfigs, err := cluster.PluginConfig().List(context.Background())
	if err != nil {
		return err
	}

	pluginConfigs = types.FilterResources(labels, pluginConfigs)

	consumerGroups, err := cluster.ConsumerGroup().List(context.Background())
	if err != nil {
		return err
	}

	consumerGroups = types.FilterResources(labels, consumerGroups)

	pluginMetadatas, err := cluster.PluginMetadata().List(context.Background())
	if err != nil {
		return err
	}

	streamRoutes, err := cluster.StreamRoute().List(context.Background())
	if err != nil {
		return err
	}

	streamRoutes = types.FilterResources(labels, streamRoutes)

	upstreams, err := cluster.Upstream().List(context.Background())
	if err != nil {
		return err
	}

	upstreams = types.FilterResources(labels, upstreams)

	conf := &types.Configuration{
		Routes:          routes,
		Services:        svcs,
		Consumers:       consumers,
		SSLs:            ssls,
		GlobalRules:     globalRules,
		PluginConfigs:   pluginConfigs,
		ConsumerGroups:  consumerGroups,
		PluginMetadatas: pluginMetadatas,
		StreamRoutes:    streamRoutes,
		Upstreams:       upstreams,
	}

	if len(labels) > 0 {
		conf.Meta = &types.ConfigurationMeta{
			Mode:   types.ModePartial,
			Labels: labels,
		}
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

		_, err = fmt.Printf("%s", data)
		if err != nil {
			return err
		}
	}

	return nil
}
