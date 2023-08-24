package suites

import (
	"bytes"
	"os/exec"

	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"
)

var _ = ginkgo.Describe("`adc dump` tests", func() {
	ginkgo.Context("Test the dump command", func() {
		_ = NewScaffold()
		ginkgo.It("should dump APISIX resources", func() {
			var syncOutput bytes.Buffer
			cmd := exec.Command("adc", "sync", "-f", "testdata/test.yaml")
			cmd.Stdout = &syncOutput
			err := cmd.Run()
			gomega.Expect(err).To(gomega.BeNil())

			var out bytes.Buffer
			cmd = exec.Command("adc", "dump", "-o", "/dev/stdout")
			cmd.Stdout = &out
			err = cmd.Run()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(out.String()).To(gomega.Equal(`name: ""
routes:
- id: route1
  methods:
  - GET
  name: route1
  service_id: svc1
  uri: /get
services:
- hosts:
  - foo.com
  id: svc1
  name: svc1
  upstream:
    hash_on: vars
    id: httpbin
    name: httpbin
    nodes:
    - host: httpbin
      port: 80
      weight: 1
    scheme: http
    type: roundrobin
version: ""
`))
		})
	})
})
