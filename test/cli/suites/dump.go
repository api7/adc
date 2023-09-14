package suites

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/test/cli/config"
	"github.com/api7/adc/test/cli/scaffold"
)

var _ = ginkgo.Describe("`adc dump` tests", func() {
	ginkgo.Context("Test the dump command", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should dump APISIX resources", func() {
			_, err := s.Sync("testdata/test.yaml")
			gomega.Expect(err).To(gomega.BeNil(), "check sync command")

			out, err := s.Dump()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(out).To(gomega.Equal(config.ReplaceUpstream(`name: ""
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
    - host: HTTPBIN_PLACEHOLDER
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
    - host: HTTPBIN_PLACEHOLDER
      port: 80
      weight: 1
    scheme: http
    type: roundrobin
version: ""
`)))

			err = s.DeleteRoute("route1")
			gomega.Expect(err).To(gomega.BeNil(), "check route delete")
			err = s.DeleteRoute("route2")
			gomega.Expect(err).To(gomega.BeNil(), "check route delete")
			err = s.DeleteService("svc1")
			gomega.Expect(err).To(gomega.BeNil(), "check service delete")
			_ = s.DeleteService("svc2")
		})
	})
})
