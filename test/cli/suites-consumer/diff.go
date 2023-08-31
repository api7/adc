package consumer

import (
	"bytes"
	"os/exec"

	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/test/cli/scaffold"
)

var _ = ginkgo.Describe("`adc diff` consumer tests", func() {
	ginkgo.Context("Basic functions", func() {
		_ = scaffold.NewScaffold()
		ginkgo.It("should return the diff result", func() {
			var out bytes.Buffer
			cmd := exec.Command("adc", "diff", "-f", "suites-consumer/testdata/test.yaml")
			cmd.Stdout = &out
			err := cmd.Run()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(out.String()).To(gomega.Equal(`creating consumer: "jack"
Summary: created 1, updated 0, deleted 0
`))
		})
	})
})
