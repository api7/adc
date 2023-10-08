package cli

import (
	"testing"

	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	_ "github.com/api7/adc/test/cli/suites-basic"
	_ "github.com/api7/adc/test/cli/suites-consumer"
	_ "github.com/api7/adc/test/cli/suites-consumer-group"
	_ "github.com/api7/adc/test/cli/suites-global-rule"
	_ "github.com/api7/adc/test/cli/suites-plugin-config"
	_ "github.com/api7/adc/test/cli/suites-plugin-metadata"
	_ "github.com/api7/adc/test/cli/suites-usecase"
)

// once apisix updated with stream_route service_id, enable this
//_ "github.com/api7/adc/test/cli/suites-stream-route"

func TestADC(t *testing.T) {
	gomega.RegisterFailHandler(ginkgo.Fail)
	ginkgo.RunSpecs(t, "ADC CLI test suites")
}
