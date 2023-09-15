package cmd

import (
	"os"

	"github.com/fatih/color"
)

func checkConfig() {
	if rootConfig.Server == "" || rootConfig.Token == "" {
		color.Yellow("ADC isn't configured, run `adc configure` to configure ADC.")
		os.Exit(0)
	}
}
