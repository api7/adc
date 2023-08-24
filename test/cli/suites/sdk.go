package suites

import (
	"context"

	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/pkg/api/apisix"
	"github.com/api7/adc/pkg/api/apisix/types"
)

var _ = ginkgo.Describe("adc APISIX SDK tests", func() {
	ginkgo.Context("Basic functions", func() {
		s := NewScaffold()
		ginkgo.It("Route resource", func() {
			cluster := s.cluster
			ctx := context.Background()

			var (
				err   error
				route *types.Route
			)

			// create service
			_, err = cluster.Service().Create(ctx, &types.Service{
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
			_, err = cluster.Route().Create(ctx, baseRoute1)

			// get route 1
			route, err = cluster.Route().Get(ctx, "route1")
			gomega.Expect(err).To(gomega.BeNil())
			assertRouteEqual(route, baseRoute1)

			// create route 2
			baseRoute2 := &types.Route{
				ID:        "route2",
				Name:      "route2",
				Uri:       "/route2",
				ServiceID: "svc1",
			}
			route, err = cluster.Route().Create(ctx, baseRoute2)
			gomega.Expect(err).To(gomega.BeNil())
			assertRouteEqual(route, baseRoute2)

			// test list
			routes, err := cluster.Route().List(ctx)
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
			_, err = cluster.Route().Update(ctx, baseRoute1)
			gomega.Expect(err).To(gomega.BeNil())

			route, err = cluster.Route().Get(ctx, "route1")
			gomega.Expect(err).To(gomega.BeNil())
			assertRouteEqual(route, baseRoute1)

			// delete route 2
			err = cluster.Route().Delete(ctx, "route2")
			gomega.Expect(err).To(gomega.BeNil())

			_, err = cluster.Route().Get(ctx, "route2")
			gomega.Expect(err).To(gomega.Equal(apisix.ErrNotFound))

			// delete route 1
			err = cluster.Route().Delete(ctx, "route1")
			gomega.Expect(err).To(gomega.BeNil())

			_, err = cluster.Route().Get(ctx, "route1")
			gomega.Expect(err).To(gomega.Equal(apisix.ErrNotFound))

			// final list
			routes, err = cluster.Route().List(ctx)
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(len(routes)).To(gomega.Equal(0))

			// delete service
			err = cluster.Service().Delete(ctx, "svc1")
			gomega.Expect(err).To(gomega.BeNil())
		})
	})
})
