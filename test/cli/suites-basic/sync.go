package basic

import (
	"net/http"

	"github.com/gavv/httpexpect/v2"
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/test/config"
	"github.com/api7/adc/test/scaffold"
)

var _ = ginkgo.Describe("`adc sync` tests", func() {
	var (
		service = &types.Service{
			ID:   "svc",
			Name: "svc",
			Hosts: []string{
				"foo.com",
			},
			Upstream: &types.Upstream{
				ID:   "httpbin",
				Name: "httpbin",
				Nodes: []types.UpstreamNode{
					{
						Host:   config.TestUpstream,
						Port:   80,
						Weight: 1,
					},
				},
			},
		}

		service1 = &types.Service{
			ID:   "svc1",
			Name: "svc1",
			Hosts: []string{
				"bar.com",
			},
			Upstream: &types.Upstream{
				ID:   "httpbin",
				Name: "httpbin",
				Nodes: []types.UpstreamNode{
					{
						Host:   config.TestUpstream,
						Port:   80,
						Weight: 1,
					},
				},
			},
		}

		route = &types.Route{
			ID:   "route",
			Name: "route",
			Uri:  "/get",
			Methods: []string{
				"GET",
			},
			ServiceID: "svc",
		}

		route1 = &types.Route{
			ID:   "route1",
			Name: "route1",
			Uri:  "/get",
			Methods: []string{
				"GET",
			},
			ServiceID: "svc1",
		}
	)

	ginkgo.Context("Basic functions", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should sync data to APISIX", func() {
			expect := httpexpect.Default(ginkgo.GinkgoT(), "http://127.0.0.1:9080")

			_, err := s.UpdateService(service)
			gomega.Expect(err).To(gomega.BeNil(), "check service update")
			_, err = s.UpdateRoute(route)
			gomega.Expect(err).To(gomega.BeNil(), "check route update")

			_, err = s.UpdateService(service1)
			gomega.Expect(err).To(gomega.BeNil(), "check service update")
			_, err = s.UpdateRoute(route1)
			gomega.Expect(err).To(gomega.BeNil(), "check route update")

			expect.GET("/get").WithHost("bar.com").Expect().Status(http.StatusOK).Body().Contains(`"Host": "bar.com"`)
			expect.GET("/get").WithHost("foo.com").Expect().Status(http.StatusOK).Body().Contains(`"Host": "foo.com"`)

			/*
				service is deleted
				service1 is updated
				service2 is created
				route is deleted
				route1 is updated
				route2 is created
			*/
			output, err := s.Sync("suites-basic/testdata/test.yaml")
			gomega.Expect(err).To(gomega.BeNil(), "check sync command")
			gomega.Expect(output).To(gomega.ContainSubstring("Summary: created 2, updated 2, deleted 2"))

			expect.GET("/get").WithHost("foo.com").Expect().Status(http.StatusNotFound)
			expect.GET("/get").WithHost("bar.com").Expect().Status(http.StatusNotFound)
			expect.GET("/get").WithHost("foo1.com").Expect().Status(http.StatusOK).Body().Contains(`"Host": "foo1.com"`)
			expect.GET("/get").WithHost("svc.com").Expect().Status(http.StatusOK).Body().Contains(`"Host": "svc.com"`)

			err = s.DeleteRoute("route1")
			gomega.Expect(err).To(gomega.BeNil(), "check route delete")
			err = s.DeleteRoute("route2")
			gomega.Expect(err).To(gomega.BeNil(), "check route delete")
			err = s.DeleteService("svc1")
			gomega.Expect(err).To(gomega.BeNil(), "check service delete")
			err = s.DeleteService("svc2")
			gomega.Expect(err).To(gomega.BeNil(), "check service delete")
			expect.GET("/get").WithHost("foo1.com").Expect().Status(http.StatusNotFound)
			expect.GET("/get").WithHost("svc.com").Expect().Status(http.StatusNotFound)
		})
	})

	ginkgo.Context("Test the sync command order", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should sync data to APISIX", func() {
			expect := httpexpect.Default(ginkgo.GinkgoT(), "http://127.0.0.1:9080")

			_, err := s.UpdateService(service)
			gomega.Expect(err).To(gomega.BeNil(), "check service update")
			_, err = s.UpdateRoute(route)
			gomega.Expect(err).To(gomega.BeNil(), "check route update")
			expect.GET("/get").WithHost("foo.com").Expect().Status(http.StatusOK).Body().Contains(`"Host": "foo.com"`)
			_, err = s.UpdateService(service1)
			gomega.Expect(err).To(gomega.BeNil(), "check service update")
			expect.GET("/get").WithHost("bar.com").Expect().Status(http.StatusNotFound)

			// Now we have two services and one route in APISIX
			// route => svc, and svc1
			// we delete svc1 and update the route to reference svc1
			output, err := s.Sync("suites-basic/testdata/test2.yaml")
			gomega.Expect(err).To(gomega.BeNil(), "check sync command")
			gomega.Expect(output).To(gomega.ContainSubstring("Summary: created 0, updated 2, deleted 1"))

			expect.GET("/get").WithHost("svc.com").Expect().Status(http.StatusOK).Body().Contains(`"Host": "svc.com"`)
			expect.GET("/get").WithHost("foo.com").Expect().Status(http.StatusNotFound)
			expect.GET("/get").WithHost("bar.com").Expect().Status(http.StatusNotFound)

			err = s.DeleteRoute("route")
			gomega.Expect(err).To(gomega.BeNil(), "check route delete")
			err = s.DeleteService("svc1")
			gomega.Expect(err).To(gomega.BeNil(), "check svc delete")
		})
	})

	ginkgo.Context("Test sync in partial mode", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should sync data to APISIX", func() {
			expect := httpexpect.Default(ginkgo.GinkgoT(), "http://127.0.0.1:9080")

			_, err := s.UpdateService(service)
			gomega.Expect(err).To(gomega.BeNil(), "check service update")
			_, err = s.UpdateRoute(route)
			gomega.Expect(err).To(gomega.BeNil(), "check route update")
			expect.GET("/get").WithHost("foo.com").Expect().Status(http.StatusOK).Body().Contains(`"Host": "foo.com"`)
			_, err = s.UpdateService(service1)
			gomega.Expect(err).To(gomega.BeNil(), "check service update")
			expect.GET("/get").WithHost("bar.com").Expect().Status(http.StatusNotFound)

			output, err := s.Sync("suites-basic/testdata/test2.yaml")
			gomega.Expect(err).To(gomega.BeNil(), "check sync command")
			gomega.Expect(output).To(gomega.ContainSubstring("Summary: created 0, updated 2, deleted 1"))

			output, err = s.Sync("suites-basic/testdata/test-partial.yaml")
			gomega.Expect(err).To(gomega.BeNil(), "check sync command")
			gomega.Expect(output).To(gomega.ContainSubstring("Summary: created 2, updated 0, deleted 0"))
		})
	})

	ginkgo.Context("Test sync in multiple file mode", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should sync data to APISIX", func() {
			output, err := s.Sync("suites-basic/testdata/test.yaml", "suites-basic/testdata/test-partial.yaml")
			gomega.Expect(err).To(gomega.BeNil(), "check sync command")
			gomega.Expect(output).To(gomega.ContainSubstring("Summary: created 6, updated 0, deleted 0"))
		})
	})

	ginkgo.Context("Test sync with common labels", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should sync data to APISIX", func() {
			output, err := s.Sync("suites-basic/testdata/test-with-common-labels.yaml")
			gomega.Expect(err).To(gomega.BeNil(), "check sync command")
			gomega.Expect(output).To(gomega.ContainSubstring("Summary: created 4, updated 0, deleted 0"))

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
  labels:
    a: "1"
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

		})
	})
})
