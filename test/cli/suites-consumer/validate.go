package consumer

import (
	"bytes"
	"os/exec"

	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/test/cli/scaffold"
)

var _ = ginkgo.Describe("`adc validate` consumer tests", func() {
	ginkgo.Context("Basic functions", func() {
		_ = scaffold.NewScaffold()
		ginkgo.It("should validate consumer schema", func() {
			var validateOutput bytes.Buffer
			cmd := exec.Command("adc", "validate", "-f", "suites-consumer/testdata/test.yaml")
			cmd.Stdout = &validateOutput
			err := cmd.Run()
			gomega.Expect(err).To(gomega.BeNil())

			gomega.Expect(validateOutput.String()).To(gomega.Equal("Get file content success: config name: , version: , consumers: 1.\nValidate file content success\n"))
		})
	})
})
