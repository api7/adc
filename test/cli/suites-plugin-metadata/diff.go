package plugin_config

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/test/scaffold"
)

var _ = ginkgo.Describe("`adc diff` plugin metadata tests", func() {
	ginkgo.Context("Basic functions", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should return the diff result", func() {
			out, err := s.Diff("suites-plugin-metadata/testdata/test.yaml")
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(out).To(gomega.Equal(`+++ plugin_metadata: "http-logger"
Summary: create 1, update 0, delete 0
`))
		})
	})
})
