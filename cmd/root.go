/*
Copyright © 2023 API7.ai
*/
package cmd

import (
	"context"
	"os"

	"github.com/fatih/color"
	homedir "github.com/mitchellh/go-homedir"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"

	"github.com/api7/adc/pkg/api/apisix"
	"github.com/api7/adc/pkg/config"
)

type Config struct {
	config.ClientConfig
	APISIXCluster apisix.Cluster
}

var (
	cfgFile    string
	rootConfig Config
)

// rootCmd represents the base command when called without any subcommands
func newRootCmd() *cobra.Command {
	rootCmd := &cobra.Command{
		Use:   "adc",
		Short: "APISIX Declarative CLI",
		Long: `A command line interface for configuring APISIX declaratively.

It can be used to validate, dump, diff, and sync configurations with an APISIX instance.
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
	rootCmd.AddCommand(newVersionCmd())
	rootCmd.AddCommand(newOpenAPI2APISIXCmd())
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
	if cfgFile == "" {
		home, err := homedir.Dir()
		if err != nil {
			color.Red("Failed to get home dir: %s", err.Error())
			os.Exit(1)
		}
		viper.AddConfigPath(home)
		viper.SetConfigName(".adc")
		viper.SetConfigType("yaml")
		cfgFile = home + "/.adc.yaml"
	} else {
		viper.SetConfigFile(cfgFile)
	}

	_, err := os.Stat(cfgFile)

	if err != nil {
		if os.IsNotExist(err) {
			color.Yellow("Configuration file %s doesn't exist.", cfgFile)
			return
		} else {
			color.Red("Failed to read configuration file: %s", err.Error())
			return
		}
	}

	err = viper.ReadInConfig()
	if err != nil {
		color.Red("Failed to read configuration file: %s", err.Error())
		return
	}

	rootConfig.Server = viper.GetString("server")
	rootConfig.Token = viper.GetString("token")
	rootConfig.CAPath = viper.GetString("capath")
	rootConfig.Certificate = viper.GetString("cert")
	rootConfig.CertificateKey = viper.GetString("cert-key")
	rootConfig.Insecure = viper.GetBool("insecure")
	cluster, err := apisix.NewCluster(context.Background(), rootConfig.ClientConfig)
	if err != nil {
		color.RedString("Failed to create a new cluster: %v", err.Error())
		return
	}
	rootConfig.APISIXCluster = cluster
}
