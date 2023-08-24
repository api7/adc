/*
Copyright © 2023 API7.ai
*/
package cmd

import (
	"context"

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
		Short: "Validate the configurations",
		Long:  `The validate command can be used to validate the configurations.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			checkConfig()

			file, err := cmd.Flags().GetString("file")
			if err != nil {
				color.Red("Get file path failed: %v", err)
				return err
			}
			if file == "" {
				color.Red("Input path is empty. Example: adc validate -f config.yaml")
				return nil
			}

			d, err := common.GetContentFromFile(file)
			if err != nil {
				color.Red("Get file content failed: %v", err)
				return err
			}

			color.Green("Get file content success: config name: %v, version: %v, routes: %v, services: %v.", d.Name, d.Name, len(d.Routes), len(d.Services))

			err = validateContent(d)
			if err != nil {
				color.Red("Command failed: %v", err)
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
	if errs != nil && len(errs) > 0 {
		color.Red("Some validation failed:")
		for _, err := range errs {
			color.Red(err.Error())
		}
	} else {
		color.Green("Validate file content success")
	}
	return nil
}
