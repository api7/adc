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
		Short: "Verify connectivity with APISIX",
		Long:  `Pings the configured APISIX instance to verify connectivity.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			checkConfig()

			return pingAPISIX()
		},
	}

	return cmd
}

// pingAPISIX check the connection to the APISIX
func pingAPISIX() error {
	cluster, err := apisix.NewCluster(context.Background(), rootConfig.ClientConfig)
	if err != nil {
		return err
	}

	err = cluster.Route().Validate(context.Background(), &types.Route{
		ID:         "test",
		Name:       "test",
		Uri:        "*",
		UpstreamId: "abcd",
	})
	if err != nil {
		color.Red("Failed to ping APISIX: %v", err.Error())
	} else {
		color.Green("Connected to APISIX successfully!")
	}
	return nil
}
