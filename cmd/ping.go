/*
Copyright © 2023 API7.ai
*/
package cmd

import (
	"context"

	"github.com/fatih/color"
	"github.com/spf13/cobra"

	"github.com/api7/adc/pkg/api/apisix"
	"github.com/api7/adc/pkg/api/apisix/types"
)

// newPingCmd represents the ping command
func newPingCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "ping",
		Short: "Verify the connection to the APISIX",
		Long:  `The ping command can be used to verify the connection to the APISIX.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			checkConfig()

			return pingAPISIX()
		},
	}

	return cmd
}

// pingAPISIX check the connection to the APISIX
func pingAPISIX() error {
	cluster := apisix.NewCluster(context.Background(), rootConfig.Server, rootConfig.Token)

	err := cluster.Route().Validate(context.Background(), &types.Route{
		Uri:        "*",
		UpstreamId: "abcd",
	})
	if err != nil {
		color.Red("failed to ping APISIX: %v", err.Error())
	} else {
		color.Green("Successfully connected to APISIX")
	}
	return nil
}
