package suites_usecase

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/test/config"
	"github.com/api7/adc/test/scaffold"
)

var _ = ginkgo.Describe("`adc sync` should handle deletion", func() {
	ginkgo.Context("Test the sync command", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("sync should delete resources", func() {
			out, err := s.Sync("suites-usecase/testdata/sync-delete-init.yaml")
			gomega.Expect(err).To(gomega.BeNil(), "check sync command")
			gomega.Expect(out).To(gomega.Equal(`creating service: "svc1"
creating route: "route1"
Summary: created 2, updated 0, deleted 0
`))

			out, err = s.Dump()
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
version: ""
`)))

			out, err = s.Sync("suites-usecase/testdata/sync-delete-updated.yaml")
			gomega.Expect(err).To(gomega.BeNil(), "check sync command")
			gomega.Expect(out).To(gomega.Equal(`creating service: "svc1_changed"
creating route: "route1_changed"
deleting route: "route1"
deleting service: "svc1"
Summary: created 2, updated 0, deleted 2
`))

			out, err = s.Dump()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(out).To(gomega.Equal(config.ReplaceUpstream(`name: ""
routes:
- id: route1_changed
  methods:
  - GET
  - PUT
  name: route1_changed
  service_id: svc1_changed
  uri: /get
services:
- hosts:
  - foo1.com
  id: svc1_changed
  name: svc1_changed
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

		})
	})
})
