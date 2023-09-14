/*
Copyright © 2023 API7.ai
*/
package cmd

import (
	"context"
	"fmt"

	"github.com/fatih/color"
	"github.com/spf13/cobra"

	"github.com/api7/adc/internal/pkg/validator"
	"github.com/api7/adc/pkg/api/apisix"
	"github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/pkg/common"
)

// newValidateCmd represents the configure command
func newValidateCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "validate",
		Short: "Validate the provided configuration file",
		Long:  `Validates the provided configuration file with the connected APISIX instance.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			checkConfig()

			file, err := cmd.Flags().GetString("file")
			if err != nil {
				color.Red("Failed to get file path: %v", err)
				return err
			}
			if file == "" {
				color.Red("File path is empty. Please specify a file path: adc validate -f config.yaml")
				return nil
			}

			d, err := common.GetContentFromFile(file)
			if err != nil {
				color.Red("Failed to read configuration file: %v", err)
				return err
			}

			msg := fmt.Sprintf("Read configuration file successfully: config name: %v, version: %v", d.Name, d.Version)
			changed := false
			if len(d.Routes) > 0 {
				msg += fmt.Sprintf(", routes: %v", len(d.Routes))
				changed = true
			}
			if len(d.Services) > 0 {
				msg += fmt.Sprintf(", services: %v", len(d.Services))
				changed = true
			}
			if len(d.Consumers) > 0 {
				msg += fmt.Sprintf(", consumers: %v", len(d.Consumers))
				changed = true
			}
			if len(d.SSLs) > 0 {
				msg += fmt.Sprintf(", ssls: %v", len(d.SSLs))
				changed = true
			}
			if len(d.GlobalRules) > 0 {
				msg += fmt.Sprintf(", global_rules: %v", len(d.GlobalRules))
				changed = true
			}
			if len(d.PluginConfigs) > 0 {
				msg += fmt.Sprintf(", plugin_configs: %v", len(d.PluginConfigs))
				changed = true
			}
			if len(d.ConsumerGroups) > 0 {
				msg += fmt.Sprintf(", consumer_groups: %v", len(d.ConsumerGroups))
				changed = true
			}
			// TODO: enable this when APISIX supports
			//if len(d.PluginMetadatas) > 0 {
			//	msg += fmt.Sprintf(", plugin_metadatas: %v", len(d.PluginMetadatas))
			//	changed = true
			//}
			if !changed {
				msg += "nothing changed"
			}
			msg += "."
			color.Green(msg)

			err = validateContent(d)
			if err != nil {
				color.Red("Failed to validate configuration file: %v", err)
				return err
			}
			return nil
		},
	}

	cmd.Flags().StringP("file", "f", "adc.yaml", "configuration file path")

	return cmd
}

// validateContent validates the content of the configuration file
func validateContent(c *types.Configuration) error {
	cluster := apisix.NewCluster(context.Background(), rootConfig.Server, rootConfig.Token)
	v, err := validator.NewValidator(c, cluster)
	if err != nil {
		color.Red("Failed to create validator: %v", err)
		return err
	}
	errs := v.Validate()
	if len(errs) > 0 {
		color.Red("Some validation failed:")
		for _, err := range errs {
			color.Red(err.Error())
		}
	} else {
		color.Green("Successfully validated configuration file!")
	}
	return nil
}
