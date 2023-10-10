package basic

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/test/scaffold"
)

var _ = ginkgo.Describe("`adc dump` tests", func() {
	ginkgo.Context("Test the dump command", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should dump APISIX resources", func() {
			_, err := s.Sync("suites-stream-route/testdata/test.yaml")
			gomega.Expect(err).To(gomega.BeNil(), "check sync command")

			out, err := s.Dump()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(out).To(gomega.Equal(`name: ""
services:
- id: svc1
  name: svc1
  upstream:
    hash_on: vars
    id: httpbin
    name: httpbin
    nodes:
    - host: 127.0.0.1
      port: 3306
      weight: 1
    pass_host: pass
    scheme: http
    type: roundrobin
stream_routes:
- id: "1"
  server_port: 9100
  service_id: svc1
version: ""
`))

			err = s.DeleteStreamRoute("1")
			gomega.Expect(err).To(gomega.BeNil(), "check stream_route delete")
			err = s.DeleteService("svc1")
			gomega.Expect(err).To(gomega.BeNil(), "check service delete")
		})
	})
})
