package suites

import (
	"bytes"
	"os/exec"

	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"
)

var _ = ginkgo.Describe("`adc validate` tests", func() {
	ginkgo.Context("Basic functions", func() {
		_ = NewScaffold()
		ginkgo.It("should validate schema", func() {
			var pingOutput bytes.Buffer
			cmd := exec.Command("adc", "validate", "-f", "testdata/test.yaml")
			cmd.Stdout = &pingOutput
			err := cmd.Run()
			gomega.Expect(err).To(gomega.BeNil())

			gomega.Expect(pingOutput.String()).To(gomega.Equal("Get file content success: config name: test, version: test, routes: 1, services: 1.\nValidate file content success\n"))
		})
	})
})
