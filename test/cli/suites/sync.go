package suites

import (
	"bytes"
	"net/http"
	"os/exec"

	"github.com/gavv/httpexpect/v2"
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"
)

var _ = ginkgo.Describe("`adc sync` tests", func() {
	ginkgo.Context("Basic functions", func() {
		s := NewScaffold()
		ginkgo.It("should sync data to APISIX", func() {
			var syncOutput bytes.Buffer
			cmd := exec.Command("adc", "sync", "-f", "testdata/test.yaml")
			cmd.Stdout = &syncOutput
			err := cmd.Run()
			gomega.Expect(err).To(gomega.BeNil())
			expect := httpexpect.Default(ginkgo.GinkgoT(), "http://127.0.0.1:9080")
			expect.GET("/get").WithHost("foo.com").Expect().Status(http.StatusOK).Body().Contains(`"Host": "foo.com"`)

			err = s.DeleteRoute("route1")
			gomega.Expect(err).To(gomega.BeNil())
			err = s.DeleteService("svc1")
			gomega.Expect(err).To(gomega.BeNil())
		})
	})
})
