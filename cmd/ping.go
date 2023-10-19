/*
Copyright © 2023 API7.ai
*/
package cmd

import (
	"io"
	"net/http"

	"github.com/fatih/color"
	"github.com/spf13/cobra"
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
	if rootConfig.Server == "" || rootConfig.Token == "" {
		color.Yellow("adc not configured, you can use `adc configure` first: " + rootConfig.Server + ", " + rootConfig.Token)
		return nil
	}

	httpClient := &http.Client{}
	req, err := http.NewRequest("GET", rootConfig.Server+"/apisix/admin/routes", nil)
	if err != nil {
		color.Red("Failed to connect to backend")
		return err
	}
	req.Header.Set("X-API-KEY", rootConfig.Token)
	resp, err := httpClient.Do(req)
	if err != nil {
		color.Red("Failed to connect to backend")
		return err
	}

	if resp.StatusCode != http.StatusOK {
		body, err := readBody(resp.Body)
		if err != nil {
			body = err.Error()
		}
		color.Red("Failed to ping backend, response: \n%s", body)
	} else {
		color.Green("Connected to backend successfully!")
	}
	return nil
}
