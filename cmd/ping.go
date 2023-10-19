/*
Copyright © 2023 API7.ai
*/
package cmd

import (
	"context"
	"io"

	"github.com/fatih/color"
	"github.com/spf13/cobra"

	"github.com/api7/adc/pkg/api/apisix"
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

func readBody(r io.ReadCloser) (string, error) {
	defer r.Close()

	data, err := io.ReadAll(r)
	if err != nil {
		return "", err
	}
	return string(data), nil
}

// pingAPISIX check the connection to the APISIX
func pingAPISIX() error {
	cluster, err := apisix.NewCluster(context.Background(), rootConfig.ClientConfig)
	if err != nil {
		return err
	}

	err = cluster.Ping()
	if err != nil {
		color.Red("Failed to ping backend, response: %s", err.Error())
	} else {
		color.Green("Connected to backend successfully!")
	}
	return nil
}
