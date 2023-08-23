/*
Copyright © 2023 API7.ai
*/
package cmd

import (
	"github.com/fatih/color"
	"github.com/spf13/cobra"

	"github.com/api7/adc/internal/pkg/validator"
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
			file, err := cmd.Flags().GetString("file")
			if err != nil {
				color.Red("Get file path failed: %v", err)
				return err
			}

			d, err := common.GetContentFromFile(file)
			if err != nil {
				color.Red("Get file content failed: %v", err)
				return err
			}

			color.Green("Get file content success: %v", d)

			err = validateContent(d)
			if err != nil {
				color.Red("Validate file content failed: %v", err)
				return err
			}

			color.Green("Validate file content success")
			return nil
		},
	}

	cmd.Flags().StringP("file", "f", "adc.yaml", "configuration file path")

	return cmd
}

// validateContent validates the content of the configuration file
func validateContent(c *types.Configuration) error {
	v, err := validator.NewValidator(c)
	if err != nil {
		color.Red("Failed to create validator: %v", err)
		return err
	}
	v.Validate()
	return nil
}
