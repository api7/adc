/*
Copyright © 2023 API7.ai
*/
package cmd

import (
	"context"
	"encoding/json"
	"fmt"

	"github.com/fatih/color"
	"github.com/spf13/cobra"

	"github.com/api7/adc/pkg/api/apisix"
)

// newDumpCmd represents the dump command
func newDumpCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "dump",
		Short: "Dump the configurations of API7",
		Long:  `The dump command can be used to dump the configurations to the API7.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			testClient()

			color.Green("Successfully dump configurations")
			return nil
		},
	}

	cmd.Flags().StringP("output", "o", "", "output file path")

	return cmd
}

func testClient() {
	cluster := apisix.NewCluster(context.Background(), rootConfig.Server, rootConfig.Token)

	ups, err := cluster.Service().List(context.Background())
	if err != nil {
		color.Red(err.Error())
		return
	}

	data, err := json.MarshalIndent(ups, "", "  ")
	if err != nil {
		color.Red(err.Error())
		return
	}

	fmt.Println(string(data))
}
