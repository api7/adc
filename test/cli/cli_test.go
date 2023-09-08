package cli

import (
	"testing"

	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	_ "github.com/api7/adc/test/cli/suites"
	_ "github.com/api7/adc/test/cli/suites-consumer"
	_ "github.com/api7/adc/test/cli/suites-global-rule"
	_ "github.com/api7/adc/test/cli/suites-plugin-config"
)

func TestADC(t *testing.T) {
	gomega.RegisterFailHandler(ginkgo.Fail)
	ginkgo.RunSpecs(t, "ADC CLI test suites")
}
