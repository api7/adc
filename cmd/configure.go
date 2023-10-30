/*
Copyright © 2023 API7.ai
*/
package cmd

import (
	"bufio"
	"crypto/tls"
	"crypto/x509"
	"errors"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"syscall"

	"github.com/fatih/color"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
	"golang.org/x/term"
)

// newConfigureCmd represents the configure command
func newConfigureCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "configure",
		Short: "Configure ADC with APISIX instance",
		Long:  `Configures ADC with APISIX's server address and token.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			return saveConfiguration(cmd)
		},
	}

	cmd.Flags().BoolP("overwrite", "f", false, "overwrite existed configuration file")

	cmd.Flags().StringP("address", "a", "", "APISIX server address")

	cmd.Flags().StringP("token", "t", "", "APISIX token")
	cmd.Flags().String("capath", "", "ca path for mtls connection")
	cmd.Flags().String("cert", "", "certificate for mtls connection")
	cmd.Flags().String("cert-key", "", "certificate key for mtls connection")
	cmd.Flags().BoolP("insecure", "k", false, "insecure connection for mtls connection")

	return cmd
}

func saveConfiguration(cmd *cobra.Command) error {
	overwrite, err := cmd.Flags().GetBool("overwrite")
	if err != nil {
		color.Red("Failed to get key: %v", err)
		return err
	}

	if !overwrite && rootConfig.Server != "" && rootConfig.Token != "" {
		color.Yellow("ADC configured. Run `adc ping` to test the configuration, or pass `-f` to overwrite configuration file.")
		return nil
	}

	rootConfig.Server, err = cmd.Flags().GetString("address")
	if err != nil {
		color.Red("Failed to get APISIX address: %v", err)
		return err
	}

	rootConfig.Token, err = cmd.Flags().GetString("token")
	if err != nil {
		color.Red("Failed to get token: %v", err)
		return err
	}

	rootConfig.CAPath, err = cmd.Flags().GetString("capath")
	if err != nil {
		color.Red("Failed to get ca path: %v", err)
		return err
	}

	rootConfig.Certificate, err = cmd.Flags().GetString("cert")
	if err != nil {
		color.Red("Failed to get certificate path: %v", err)
		return err
	}
	rootConfig.CertificateKey, err = cmd.Flags().GetString("cert-key")
	if err != nil {
		color.Red("Failed to get certificate key path: %v", err)
		return err
	}
	rootConfig.Insecure, err = cmd.Flags().GetBool("insecure")
	if err != nil {
		color.Red("Failed to get insecure option: %v", err)
		return err
	}

	if rootConfig.CAPath != "" {
		if rootConfig.Certificate != "" && rootConfig.CertificateKey == "" {
			color.Red("Certificate key file path no provided!")
			return errors.New("certificate key file path no provided")
		}

		if rootConfig.Certificate == "" && rootConfig.CertificateKey != "" {
			color.Red("Certificate file path no provided!")
			return errors.New("certificate file path no provided")
		}

		rootConfig.CAPath, err = filepath.Abs(rootConfig.CAPath)
		if err != nil {
			color.Red("Failed to resolve CA path: %v", err)
			return err
		}
		rootConfig.Certificate, err = filepath.Abs(rootConfig.Certificate)
		if err != nil {
			color.Red("Failed to resolve certificate path: %v", err)
			return err
		}
		rootConfig.CertificateKey, err = filepath.Abs(rootConfig.CertificateKey)
		if err != nil {
			color.Red("Failed to resolve certificate key path: %v", err)
			return err
		}

		if strings.HasPrefix(rootConfig.Server, "http://") {
			color.Yellow("APISIX address is configured with HTTP protocol, replaced by HTTPS")
			rootConfig.Server = strings.Replace(rootConfig.Server, "http://", "https://", 1)
		}

		rootCA, err := os.ReadFile(rootConfig.CAPath)
		if err != nil {
			color.Red("Failed to read CA file: %v", err)
			return err
		}

		caCertPool := x509.NewCertPool()
		ok := caCertPool.AppendCertsFromPEM(rootCA)
		if !ok {
			color.Red("Failed to parse CA certificate")
			return errors.New("failed to parse CA certificate")
		}

		cert, err := os.ReadFile(rootConfig.Certificate)
		if err != nil {
			color.Red("Failed to read certificate file: %v", err)
			return err
		}
		key, err := os.ReadFile(rootConfig.CertificateKey)
		if err != nil {
			color.Red("Failed to read certificate key file: %v", err)
			return err
		}
		_, err = tls.X509KeyPair(cert, key)
		if err != nil {
			color.Red("Failed to parse x509 key pair: %v", err)
			return err
		}
	}

	reader := bufio.NewReader(os.Stdin)
	if rootConfig.Server == "" {
		fmt.Println("Please enter the APISIX server address: ")
		server, err := reader.ReadString('\n')
		if err != nil {
			return err
		}
		rootConfig.Server = strings.TrimSpace(server)
	}

	if !strings.HasPrefix(rootConfig.Server, "http://") && !strings.HasPrefix(rootConfig.Server, "https://") {
		color.Yellow("APISIX address " + rootConfig.Server + " is configured without protocol, using HTTP")
		rootConfig.Server = "http://" + rootConfig.Server
	}
	rootConfig.Server = strings.TrimSuffix(rootConfig.Server, "/")

	_, err = url.Parse(rootConfig.Server)
	if err != nil {
		color.Red("Parse APISIX server address failed: %v", err)
		return err
	}

	if rootConfig.Token == "" || overwrite {
		fmt.Println("Please enter the APISIX token: ")
		if term.IsTerminal(syscall.Stdin) {
			token, err := term.ReadPassword(syscall.Stdin)
			if err != nil {
				return err
			}
			rootConfig.Token = strings.TrimSpace(string(token))
		} else {
			token, err := reader.ReadString('\n')
			if err != nil {
				return err
			}
			rootConfig.Token = strings.TrimSpace(string(token))
		}
	}

	// use viper to save the configuration
	viper.Set("server", rootConfig.Server)
	viper.Set("token", rootConfig.Token)
	viper.Set("capath", rootConfig.CAPath)
	viper.Set("cert", rootConfig.Certificate)
	viper.Set("cert-key", rootConfig.CertificateKey)
	viper.Set("insecure", rootConfig.Insecure)

	if overwrite {
		// because WriteConfig fails to write if the file does not exist
		// and WriteConfigAs does write even if the file does not exist
		// see: https://github.com/spf13/viper/issues/433
		err = viper.WriteConfigAs(cfgFile)
	} else {
		err = viper.SafeWriteConfig()
	}
	if err != nil {
		color.Red("Failed to configure ADC")
		return err
	}

	color.Green("ADC configured successfully!")

	return pingAPISIX()
}
