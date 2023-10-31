package basic

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/test/config"
	"github.com/api7/adc/test/scaffold"
)

var _ = ginkgo.Describe("`adc dump` tests", func() {
	ginkgo.Context("Test the dump command", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should dump APISIX resources", func() {
			_, err := s.Sync("suites-basic/testdata/test.yaml")
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
  priority: 0
  service_id: svc1
  status: 1
  uri: /get
- id: route2
  methods:
  - GET
  name: route2
  priority: 0
  service_id: svc2
  status: 1
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
      priority: 0
      weight: 1
    pass_host: pass
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
      priority: 0
      weight: 1
    pass_host: pass
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
			err = s.DeleteService("svc2")
			gomega.Expect(err).To(gomega.BeNil(), "check service delete")
		})
	})

	ginkgo.Context("Test the dump command with label filter", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should dump APISIX resources", func() {
			_, err := s.Sync("suites-basic/testdata/test-with-labels.yaml")
			gomega.Expect(err).To(gomega.BeNil(), "check sync command")

			out, err := s.DumpWithLabels(types.Labels{
				"a": "1",
			})
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(out).To(gomega.Equal(config.ReplaceUpstream(`meta:
  labels:
    a: "1"
  mode: partial
name: ""
routes:
- id: route1
  labels:
    a: "1"
    b: "2"
    c: "3"
  methods:
  - GET
  - PUT
  name: route1
  priority: 0
  service_id: svc1
  status: 1
  uri: /get
- id: route2
  labels:
    a: "1"
    b: "2"
  methods:
  - GET
  name: route2
  priority: 0
  service_id: svc2
  status: 1
  uri: /get
services:
- hosts:
  - foo1.com
  id: svc1
  labels:
    a: "1"
    b: "2"
    c: "3"
  name: svc1
  upstream:
    hash_on: vars
    id: httpbin
    name: httpbin
    nodes:
    - host: httpbin.org
      port: 80
      priority: 0
      weight: 1
    pass_host: pass
    scheme: http
    type: roundrobin
- hosts:
  - svc.com
  id: svc2
  labels:
    a: "1"
    b: "2"
  name: svc2
  upstream:
    hash_on: vars
    id: httpbin
    name: httpbin
    nodes:
    - host: httpbin.org
      port: 80
      priority: 0
      weight: 1
    pass_host: pass
    scheme: http
    type: roundrobin
version: ""
`)))

			out, err = s.DumpWithLabels(types.Labels{
				"c": "3",
			})
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(out).To(gomega.Equal(config.ReplaceUpstream(`meta:
  labels:
    c: "3"
  mode: partial
name: ""
routes:
- id: route1
  labels:
    a: "1"
    b: "2"
    c: "3"
  methods:
  - GET
  - PUT
  name: route1
  priority: 0
  service_id: svc1
  status: 1
  uri: /get
services:
- hosts:
  - foo1.com
  id: svc1
  labels:
    a: "1"
    b: "2"
    c: "3"
  name: svc1
  upstream:
    hash_on: vars
    id: httpbin
    name: httpbin
    nodes:
    - host: httpbin.org
      port: 80
      priority: 0
      weight: 1
    pass_host: pass
    scheme: http
    type: roundrobin
version: ""
`)))

			out, err = s.DumpWithLabels(types.Labels{
				"b": "2",
				"c": "3",
			})
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(out).To(gomega.Equal(config.ReplaceUpstream(`meta:
  labels:
    b: "2"
    c: "3"
  mode: partial
name: ""
routes:
- id: route1
  labels:
    a: "1"
    b: "2"
    c: "3"
  methods:
  - GET
  - PUT
  name: route1
  priority: 0
  service_id: svc1
  status: 1
  uri: /get
services:
- hosts:
  - foo1.com
  id: svc1
  labels:
    a: "1"
    b: "2"
    c: "3"
  name: svc1
  upstream:
    hash_on: vars
    id: httpbin
    name: httpbin
    nodes:
    - host: httpbin.org
      port: 80
      priority: 0
      weight: 1
    pass_host: pass
    scheme: http
    type: roundrobin
version: ""
`)))

			out, err = s.DumpWithLabels(types.Labels{
				"c": "1",
			})
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(out).To(gomega.Equal(config.ReplaceUpstream(`meta:
  labels:
    c: "1"
  mode: partial
name: ""
version: ""
`)))

			out, err = s.DumpWithLabels(types.Labels{
				"b": "2",
				"c": "1",
			})
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(out).To(gomega.Equal(config.ReplaceUpstream(`meta:
  labels:
    b: "2"
    c: "1"
  mode: partial
name: ""
version: ""
`)))

			err = s.DeleteRoute("route1")
			gomega.Expect(err).To(gomega.BeNil(), "check route delete")
			err = s.DeleteRoute("route2")
			gomega.Expect(err).To(gomega.BeNil(), "check route delete")
			err = s.DeleteService("svc1")
			gomega.Expect(err).To(gomega.BeNil(), "check service delete")
			err = s.DeleteService("svc2")
			gomega.Expect(err).To(gomega.BeNil(), "check service delete")
		})
	})
})
