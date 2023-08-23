package cmd

import (
	"os"

	"github.com/fatih/color"
)

func checkConfig() {
	if rootConfig.Server == "" || rootConfig.Token == "" {
		color.Yellow("adc hasn't been configured, you can use `adc configure` to configure adc")
		os.Exit(0)
	}
}
