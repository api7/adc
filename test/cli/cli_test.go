package cli

import (
	"bytes"
	"os/exec"
	"strings"
	"testing"

	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"
)

func TestADC(t *testing.T) {
	gomega.RegisterFailHandler(ginkgo.Fail)
	ginkgo.RunSpecs(t, "ADC CLI test suites")
}

var _ = ginkgo.Describe("ADC CLI test", func() {
	ginkgo.Context("Test the diff command", func() {
		ginkgo.It("should return the diff result", func() {
			cmd := exec.Command("adc", "configure")
			cmd.Stdin = strings.NewReader("http://127.0.0.1:9180\nedd1c9f034335f136f87ad84b625c8f1\n")
			err := cmd.Run()
			gomega.Expect(err).To(gomega.BeNil())

			var out bytes.Buffer
			cmd = exec.Command("adc", "diff", "-f", "testdata/test.yaml")
			cmd.Stdout = &out
			err = cmd.Run()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(out.String()).To(gomega.Equal(`creating service: "svc1"
creating service: "svc2"
creating route: "route1"
Summary: created 3, updated 0, deleted 0
`))
		})
	})
})
