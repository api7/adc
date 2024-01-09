package consumer

import (
	"net/http"
	"time"

	"github.com/gavv/httpexpect/v2"
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/test/config"
	"github.com/api7/adc/test/scaffold"
)

var _ = ginkgo.Describe("`adc sync` tests", func() {
	var (
		user    = "jack"
		authKey = "auth-one"

		consumer = &types.Consumer{
			Username: user,
			Plugins: types.Plugins{
				"key-auth": types.Plugin{
					"key": authKey,
				},
				"limit-count": types.Plugin{
					"count":         2,
					"time_window":   60,
					"rejected_code": 503,
					"key":           "remote_addr",
				},
			},
		}

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

		route = &types.Route{
			ID:   "route",
			Name: "route",
			Uri:  "/get",
			Methods: []string{
				"GET",
			},
			ServiceID: "svc",
			Plugins: types.Plugins{
				"key-auth": types.Plugin{},
			},
		}
	)

	ginkgo.Context("Basic functions", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should sync data to APISIX", func() {
			expect := httpexpect.Default(ginkgo.GinkgoT(), "http://127.0.0.1:9080")

			_, err := s.UpdateConsumer(consumer)
			gomega.Expect(err).To(gomega.BeNil(), "check consumer update")
			_, err = s.UpdateService(service)
			gomega.Expect(err).To(gomega.BeNil(), "check service update")
			_, err = s.UpdateRoute(route)
			gomega.Expect(err).To(gomega.BeNil(), "check route update")

			time.Sleep(time.Second * 1)

			resp := expect.GET("/get").WithHeader("apikey", authKey).
				WithHost("foo.com").Expect()
			resp.Status(http.StatusOK)
			resp.Header("X-Ratelimit-Remaining").IsEqual("1")

			resp = expect.GET("/get").WithHeader("apikey", authKey).
				WithHost("foo.com").Expect().Status(http.StatusOK)
			resp.Status(http.StatusOK)
			resp.Header("X-Ratelimit-Remaining").IsEqual("0")

			resp = expect.GET("/get").WithHeader("apikey", authKey).
				WithHost("foo.com").Expect()
			resp.Status(http.StatusServiceUnavailable)
			resp.Header("X-Ratelimit-Remaining").IsEqual("0")

			err = s.DeleteRoute("route")
			gomega.Expect(err).To(gomega.BeNil(), "check route delete")
			err = s.DeleteService("svc")
			gomega.Expect(err).To(gomega.BeNil(), "check service delete")
			err = s.DeleteConsumer(user)
			gomega.Expect(err).To(gomega.BeNil(), "check consumer delete")
			expect.GET("/get").WithHost("foo.com").Expect().Status(http.StatusNotFound)
		})
	})
})
