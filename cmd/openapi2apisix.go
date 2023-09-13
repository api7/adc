package cmd

import (
	"github.com/spf13/cobra"
)

// newOpenAPI2APISIXCmd represents the openapi2apisix command
func newOpenAPI2APISIXCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "openapi2apisix",
		Short: "Convert OpenAPI file to ADC configuration file",
		Long:  `The openapi2apisix command can be used to convert OpenAPI file to ADC configuration file.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			checkConfig()

			return pingAPISIX()
		},
	}

	cmd.Flags().StringP("file", "f", "", "OpenAPI configuration file path")
	cmd.Flags().StringP("output", "o", "", "output file path")

	return cmd
}
