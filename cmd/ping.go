/*
Copyright © 2023 API7.ai
*/
package cmd

import (
	"net/http"

	"github.com/fatih/color"
	"github.com/spf13/cobra"
)

// newPingCmd represents the ping command
func newPingCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "ping",
		Short: "Verify the connection to the API7",
		Long:  `The ping command can be used to verify the connection to the API7.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			return pingAPI7()
		},
	}

	return cmd
}

// pingAPI7 check the connection to the API7
func pingAPI7() error {
	if rootConfig.Server == "" && rootConfig.Token != "" {
		color.Yellow("adc has been configured, you can use `adc ping` to test the connection")
		return nil
	}

	httpClient := &http.Client{}
	req, err := http.NewRequest("GET", rootConfig.Server, nil)
	if err != nil {
		color.Red("Failed to connect to the API7")
		return err
	}
	req.Header.Set("X-API-KEY", rootConfig.Token)
	resp, err := httpClient.Do(req)
	if err != nil {
		color.Red("Failed to connect to the API7")
		return err
	}
	if resp.StatusCode == http.StatusOK {
		color.Green("Scucessfully connected to the API7")
	} else {
		color.Red("Failed to connect to the API7")
	}
	return nil

}
