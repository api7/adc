package basic

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/test/scaffold"
)

var _ = ginkgo.Describe("`adc sync` tests", func() {
	ginkgo.Context("Basic functions", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should sync data to APISIX", func() {
			output, err := s.Sync("suites-stream-route/testdata/test.yaml")
			gomega.Expect(err).To(gomega.BeNil(), "check sync command")
			gomega.Expect(output).To(gomega.ContainSubstring(`creating service: "svc1"
creating stream_route: "1"
Summary: created 2, updated 0, deleted 0`))

			sr, err := s.GetStreamRoute("1")
			gomega.Expect(err).To(gomega.BeNil(), "check stream_route get")
			gomega.Expect(sr.ID).To(gomega.Equal("1"))
			gomega.Expect(sr.ServerPort).To(gomega.Equal(9100))
			gomega.Expect(sr.ServiceID).To(gomega.Equal("svc1"))

			svc, err := s.GetService("svc1")
			gomega.Expect(err).To(gomega.BeNil(), "check service get")
			gomega.Expect(svc.ID).To(gomega.Equal("svc1"))
			gomega.Expect(len(svc.Upstream.Nodes)).To(gomega.Equal(1))
			gomega.Expect(svc.Upstream.Nodes[0].Host).To(gomega.Equal("127.0.0.1"))
			gomega.Expect(svc.Upstream.Nodes[0].Port).To(gomega.Equal(3306))

			err = s.DeleteStreamRoute("1")
			gomega.Expect(err).To(gomega.BeNil(), "check stream_route delete")
			err = s.DeleteService("svc1")
			gomega.Expect(err).To(gomega.BeNil(), "check service delete")
		})
	})
})
