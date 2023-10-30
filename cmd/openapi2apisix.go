package cmd

import (
	"bufio"
	"context"
	"fmt"
	"io"
	"os"

	"github.com/fatih/color"
	"github.com/spf13/cobra"
	"sigs.k8s.io/yaml"

	"github.com/api7/adc/internal/pkg/openapi2apisix"
	"github.com/api7/adc/pkg/common"
)

// newOpenAPI2APISIXCmd represents the openapi2apisix command
func newOpenAPI2APISIXCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "openapi2apisix",
		Short: "Convert OpenAPI configuration to ADC configuration",
		Long:  `Converts the configuration in OpenAPI format to the ADC configuration format.`,
		RunE: func(cmd *cobra.Command, args []string) error {

			err := openAPI2APISIX(cmd)
			if err != nil {
				color.Red(err.Error())
			}
			return err
		},
	}

	cmd.Flags().StringP("file", "f", "", "OpenAPI configuration file path")
	cmd.Flags().StringP("output", "o", "/dev/stdout", "output file path")

	return cmd
}

func openAPI2APISIX(cmd *cobra.Command) error {
	output, err := cmd.Flags().GetString("output")
	if err != nil {
		color.Red("Failed to get output file path: %v", err)
		return err
	}
	if output == "" {
		output = "/dev/stdout"
	}

	save := true
	if output == "/dev/stdout" {
		save = false
	}

	filename, err := cmd.Flags().GetString("file")
	if err != nil {
		color.Red("Failed to get OpenAPI file path: %v", err)
		return err
	}
	if filename == "" {
		color.Red("OpenAPI file path is empty.")
		return nil
	}

	f, err := os.Open(filename)
	if err != nil {
		color.Red("Failed to open %s: %s", filename, err)
		return err
	}
	defer f.Close()

	reader := bufio.NewReader(f)
	fileContent, err := io.ReadAll(reader)
	if err != nil {
		color.Red("Failed to read file %s: %s", filename, err)
		return err
	}

	conf, err := openapi2apisix.Convert(context.Background(), fileContent)
	if err != nil {
		color.Red("Failed to convert OpenAPI file %s: %s", filename, err)
		return err
	}

	if save {
		err = common.SaveAPISIXConfiguration(output, conf)
		if err != nil {
			return err
		}
		color.Green("Converted OpenAPI file to %s successfully ", output)
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
