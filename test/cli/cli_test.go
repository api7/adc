package cli

import (
	"bytes"
	"net/http"
	"os/exec"
	"strings"
	"testing"

	httpexpect "github.com/gavv/httpexpect/v2"
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
creating route: "route1"
Summary: created 2, updated 0, deleted 0
`))

			var syncOutput bytes.Buffer
			cmd = exec.Command("adc", "sync", "-f", "testdata/test.yaml")
			cmd.Stdout = &syncOutput
			err = cmd.Run()
			gomega.Expect(err).To(gomega.BeNil())
			expect := httpexpect.Default(ginkgo.GinkgoT(), "http://127.0.0.1:9080")
			expect.GET("/get").WithHost("foo.com").Expect().Status(http.StatusOK).Body().Contains(`"Host": "foo.com"`)
		})
	})
})
