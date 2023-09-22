package basic

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/test/scaffold"
)

var _ = ginkgo.Describe("`adc diff` tests", func() {
	ginkgo.Context("Basic functions", func() {
		s := scaffold.NewMtlsScaffold()
		ginkgo.It("should return the diff result", func() {
			out, err := s.Diff("suites-basic/testdata/test.yaml")
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(out).To(gomega.Equal(`+++ service: "svc1"
+++ service: "svc2"
+++ route: "route1"
+++ route: "route2"
Summary: create 4, update 0, delete 0
`))
		})
	})
})
