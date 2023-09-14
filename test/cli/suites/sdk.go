package suites

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/pkg/api/apisix"
	"github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/test/cli/scaffold"
)

var _ = ginkgo.Describe("adc APISIX SDK tests", func() {
	ginkgo.Context("Basic functions", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("Route resource", func() {
			var (
				err   error
				route *types.Route
			)

			// create service
			_, err = s.CreateService(&types.Service{
				ID:   "svc1",
				Name: "svc1",
				Hosts: []string{
					"foo.com",
				},
				Upstream: types.Upstream{
					ID:   "ups1",
					Name: "ups1",
					Nodes: []types.UpstreamNode{
						{
							Host:   "foo.com",
							Port:   80,
							Weight: 1,
						},
					},
				},
			})
			gomega.Expect(err).To(gomega.BeNil())

			// utils
			assertRouteEqual := func(expect, toBe *types.Route) {
				gomega.Expect(expect.ID).To(gomega.Equal(toBe.ID))
				gomega.Expect(expect.Name).To(gomega.Equal(toBe.Name))
				gomega.Expect(expect.Uri).To(gomega.Equal(toBe.Uri))
				gomega.Expect(expect.ServiceID).To(gomega.Equal(toBe.ServiceID))
			}

			// create route 1
			baseRoute1 := &types.Route{
				ID:        "route1",
				Name:      "route1",
				Uri:       "/route1",
				ServiceID: "svc1",
			}
			_, _ = s.CreateRoute(baseRoute1)

			// get route 1
			route, err = s.GetRoute("route1")
			gomega.Expect(err).To(gomega.BeNil())
			assertRouteEqual(route, baseRoute1)

			// create route 2
			baseRoute2 := &types.Route{
				ID:        "route2",
				Name:      "route2",
				Uri:       "/route2",
				ServiceID: "svc1",
			}
			route, err = s.CreateRoute(baseRoute2)
			gomega.Expect(err).To(gomega.BeNil())
			assertRouteEqual(route, baseRoute2)

			// test list
			routes, err := s.ListRoute()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(len(routes)).To(gomega.Equal(2))
			var route1, route2 *types.Route
			for _, r := range routes {
				if r.ID == "route1" {
					route1 = r
				} else if r.ID == "route2" {
					route2 = r
				}
			}
			gomega.Expect(route1).NotTo(gomega.BeNil())
			gomega.Expect(route2).NotTo(gomega.BeNil())

			assertRouteEqual(route1, baseRoute1)
			assertRouteEqual(route2, baseRoute2)

			// update & get route 1
			baseRoute1 = &types.Route{
				ID:        "route1",
				Name:      "route1",
				Uri:       "/route1-updated",
				ServiceID: "svc1",
			}
			_, err = s.UpdateRoute(baseRoute1)
			gomega.Expect(err).To(gomega.BeNil())

			route, err = s.GetRoute("route1")
			gomega.Expect(err).To(gomega.BeNil())
			assertRouteEqual(route, baseRoute1)

			// delete route 2
			err = s.DeleteRoute("route2")
			gomega.Expect(err).To(gomega.BeNil())

			_, err = s.GetRoute("route2")
			gomega.Expect(err).To(gomega.Equal(apisix.ErrNotFound))

			// delete route 1
			err = s.DeleteRoute("route1")
			gomega.Expect(err).To(gomega.BeNil())

			_, err = s.GetRoute("route1")
			gomega.Expect(err).To(gomega.Equal(apisix.ErrNotFound))

			// final list
			routes, err = s.ListRoute()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(len(routes)).To(gomega.Equal(0))

			// delete service
			err = s.DeleteService("svc1")
			gomega.Expect(err).To(gomega.BeNil())
		})
	})
})
