/*
Copyright © 2023 API7.ai
*/
package cmd

import (
	"os"

	"github.com/fatih/color"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"

	"github.com/api7/adc/pkg/config"
)

var (
	cfgFile    string
	rootConfig config.ClientConfig
)

// rootCmd represents the base command when called without any subcommands
func newRootCmd() *cobra.Command {
	rootCmd := &cobra.Command{
		Use:   "adc",
		Short: "API7 Declarative CLI",
		Long: `A CLI tool for API7 Declarative configurations.

It can be used to dump, diff, sync configurations to API7 server.
		`,
	}
	cobra.OnInitialize(initConfig)
	rootCmd.PersistentFlags().StringVar(&cfgFile, "config", "", "config file (default is $HOME/.adc.yaml)")

	rootCmd.AddCommand(newConfigureCmd())
	rootCmd.AddCommand(newPingCmd())
	rootCmd.AddCommand(newDumpCmd())
	rootCmd.AddCommand(newDiffCmd())
	rootCmd.AddCommand(newSyncCmd())
	rootCmd.AddCommand(newValidateCmd())
	return rootCmd
}

// Execute adds all child commands to the root command and sets flags appropriately.
// This is called by main.main(). It only needs to happen once to the rootCmd.
func Execute() {
	rootCmd := newRootCmd()
	err := rootCmd.Execute()
	if err != nil {
		os.Exit(1)
	}
}

func initConfig() {
	if cfgFile != "" {
		viper.SetConfigFile(cfgFile)
	} else {
		// set default config file $HOME/.adc.yaml
		viper.SetConfigFile("$HOME/.adc.yaml")
	}
	viper.SetConfigName(".adc")
	viper.SetConfigType("yaml")
	viper.AddConfigPath("$HOME/")
	err := viper.ReadInConfig()
	if err != nil {
		color.Red("Fatal to read config file, please run `adc configure` to configure the client first.")
	}

	rootConfig.Server = viper.GetString("server")
	rootConfig.Token = viper.GetString("token")
}
