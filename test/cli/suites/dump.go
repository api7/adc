package suites

import (
	"bytes"
	"os/exec"

	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"
)

var _ = ginkgo.Describe("`adc dump` tests", func() {
	ginkgo.Context("Test the dump command", func() {
		s := NewScaffold()
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
  - PUT
  name: route1
  service_id: svc1
  uri: /get
- id: route2
  methods:
  - GET
  name: route2
  service_id: svc2
  uri: /get
services:
- hosts:
  - foo1.com
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
- hosts:
  - svc.com
  id: svc2
  name: svc2
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

			err = s.DeleteRoute("route1")
			gomega.Expect(err).To(gomega.BeNil(), "check route delete")
			err = s.DeleteRoute("route2")
			gomega.Expect(err).To(gomega.BeNil(), "check route delete")
			err = s.DeleteService("svc1")
			gomega.Expect(err).To(gomega.BeNil(), "check service delete")
			err = s.DeleteService("svc2")
		})
	})
})
