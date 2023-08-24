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
			var validateOutput bytes.Buffer
			cmd := exec.Command("adc", "validate", "-f", "testdata/test.yaml")
			cmd.Stdout = &validateOutput
			err := cmd.Run()
			gomega.Expect(err).To(gomega.BeNil())

			gomega.Expect(validateOutput.String()).To(gomega.Equal("Get file content success: config name: test, version: test, routes: 2, services: 2.\nValidate file content success\n"))
		})
	})
})
