package suites

import (
	"bytes"
	"os/exec"

	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"
)

var _ = ginkgo.Describe("`adc diff` tests", func() {
	ginkgo.Context("Basic functions", func() {
		_ = NewScaffold()
		ginkgo.It("should return the diff result", func() {
			var out bytes.Buffer
			cmd := exec.Command("adc", "diff", "-f", "testdata/test.yaml")
			cmd.Stdout = &out
			err := cmd.Run()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(out.String()).To(gomega.Equal(`creating service: "svc1"
creating service: "svc2"
creating route: "route1"
creating route: "route2"
Summary: created 4, updated 0, deleted 0
`))
		})
	})
})
