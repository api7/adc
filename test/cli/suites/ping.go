package suites

import (
	"bytes"
	"os/exec"

	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/test/cli/scaffold"
)

var _ = ginkgo.Describe("`adc ping` tests", func() {
	ginkgo.Context("Basic functions", func() {
		_ = scaffold.NewScaffold()
		ginkgo.It("should connect to APISIX", func() {
			var pingOutput bytes.Buffer
			cmd := exec.Command("adc", "ping")
			cmd.Stdout = &pingOutput
			err := cmd.Run()
			gomega.Expect(err).To(gomega.BeNil())

			gomega.Expect(pingOutput.String()).To(gomega.Equal("Successfully connected to APISIX\n"))
		})
	})
})
