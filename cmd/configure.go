/*
Copyright © 2023 API7.ai
*/
package cmd

import (
	"bufio"
	"fmt"
	"os"
	"strings"

	"github.com/fatih/color"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
)

// newConfigureCmd represents the configure command
func newConfigureCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "configure",
		Short: "Configure the connection of API7",
		Long:  `The ping command can be used to configure the connection to the API7.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			return saveConfiguration()
		},
	}

	return cmd
}

func saveConfiguration() error {
	if rootConfig.Server != "" && rootConfig.Token != "" {
		color.Yellow("adc has been configured, you can use `adc ping` to test the connection")
		return nil
	}

	reader := bufio.NewReader(os.Stdin)
	if rootConfig.Server == "" {
		fmt.Println("Please input the server address: ")
		server, _ := reader.ReadString('\n')
		rootConfig.Server = strings.TrimSpace(server)
	}

	if rootConfig.Token == "" {

		fmt.Println("Please input the Token: ")
		token, _ := reader.ReadString('\n')
		rootConfig.Token = strings.TrimSpace(token)
	}

	// use viper to save the configuration
	viper.Set("server", rootConfig.Server)
	viper.Set("token", rootConfig.Token)
	if err := viper.SafeWriteConfig(); err != nil {
		color.Red("failed configure ADC")
		return err
	}

	color.Green("Successfully configure ADC")
	return nil
}
