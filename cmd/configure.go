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
		Short: "Configure ADC with APISIX instance",
		Long:  `Configures ADC with APISIX's server address and token.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			return saveConfiguration()
		},
	}

	return cmd
}

func saveConfiguration() error {
	if rootConfig.Server != "" && rootConfig.Token != "" {
		color.Yellow("ADC configured. Run `adc ping` to test the configuration.")
		return nil
	}

	reader := bufio.NewReader(os.Stdin)
	if rootConfig.Server == "" {
		fmt.Println("Please enter the APISIX server address: ")
		server, _ := reader.ReadString('\n')
		rootConfig.Server = strings.TrimSpace(server)
	}

	if rootConfig.Token == "" {

		fmt.Println("Please enter the token: ")
		token, _ := reader.ReadString('\n')
		rootConfig.Token = strings.TrimSpace(token)
	}

	// use viper to save the configuration
	viper.Set("server", rootConfig.Server)
	viper.Set("token", rootConfig.Token)
	if err := viper.SafeWriteConfig(); err != nil {
		color.Red("Failed to configure ADC")
		return err
	}

	color.Green("ADC configured successfully!")

	return pingAPISIX()
}
