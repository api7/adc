package basic

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/pkg/api/apisix"
	"github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/test/scaffold"
)

var _ = ginkgo.Describe("adc APISIX SDK tests", func() {
	ginkgo.Context("Basic functions", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("Route resource", func() {
			var (
				err   error
				route *types.StreamRoute
			)

			// create service
			_, err = s.CreateService(&types.Service{
				ID:   "svc1",
				Name: "svc1",
				Upstream: &types.Upstream{
					ID:   "ups1",
					Name: "ups1",
					Nodes: []types.UpstreamNode{
						{
							Host:   "127.0.0.1",
							Port:   3306,
							Weight: 1,
						},
					},
				},
			})
			gomega.Expect(err).To(gomega.BeNil())

			// utils
			assertStreamRouteEqual := func(expect, toBe *types.StreamRoute) {
				gomega.Expect(expect.ID).To(gomega.Equal(toBe.ID))
				gomega.Expect(expect.RemoteAddr).To(gomega.Equal(toBe.RemoteAddr))
				gomega.Expect(expect.ServerAddr).To(gomega.Equal(toBe.ServerAddr))
				gomega.Expect(expect.ServerPort).To(gomega.Equal(toBe.ServerPort))
				gomega.Expect(expect.ServiceID).To(gomega.Equal(toBe.ServiceID))
			}

			// create route 1
			baseStreamRoute1 := &types.StreamRoute{
				ID:         "route1",
				ServerPort: 9100,
				ServiceID:  "svc1",
			}
			_, err = s.CreateStreamRoute(baseStreamRoute1)
			gomega.Expect(err).To(gomega.BeNil())

			// get route 1
			route, err = s.GetStreamRoute("route1")
			gomega.Expect(err).To(gomega.BeNil())
			assertStreamRouteEqual(route, baseStreamRoute1)

			// create route 2
			baseStreamRoute2 := &types.StreamRoute{
				ID:         "route2",
				ServerAddr: "127.0.0.10",
				ServerPort: 9101,
				ServiceID:  "svc1",
			}
			route, err = s.CreateStreamRoute(baseStreamRoute2)
			gomega.Expect(err).To(gomega.BeNil())
			assertStreamRouteEqual(route, baseStreamRoute2)

			// test list
			routes, err := s.ListStreamRoute()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(len(routes)).To(gomega.Equal(2))
			var route1, route2 *types.StreamRoute
			for _, r := range routes {
				if r.ID == "route1" {
					route1 = r
				} else if r.ID == "route2" {
					route2 = r
				}
			}
			gomega.Expect(route1).NotTo(gomega.BeNil())
			gomega.Expect(route2).NotTo(gomega.BeNil())

			assertStreamRouteEqual(route1, baseStreamRoute1)
			assertStreamRouteEqual(route2, baseStreamRoute2)

			// update & get route 1
			baseStreamRoute1 = &types.StreamRoute{
				ID:         "route1",
				ServerPort: 9101,
				ServiceID:  "svc1",
			}
			_, err = s.UpdateStreamRoute(baseStreamRoute1)
			gomega.Expect(err).To(gomega.BeNil())

			route, err = s.GetStreamRoute("route1")
			gomega.Expect(err).To(gomega.BeNil())
			assertStreamRouteEqual(route, baseStreamRoute1)

			// delete route 2
			err = s.DeleteStreamRoute("route2")
			gomega.Expect(err).To(gomega.BeNil())

			_, err = s.GetStreamRoute("route2")
			gomega.Expect(err).To(gomega.Equal(apisix.ErrNotFound))

			// delete route 1
			err = s.DeleteStreamRoute("route1")
			gomega.Expect(err).To(gomega.BeNil())

			_, err = s.GetStreamRoute("route1")
			gomega.Expect(err).To(gomega.Equal(apisix.ErrNotFound))

			// final list
			routes, err = s.ListStreamRoute()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(len(routes)).To(gomega.Equal(0))

			// delete service
			err = s.DeleteService("svc1")
			gomega.Expect(err).To(gomega.BeNil())
		})
	})
})
