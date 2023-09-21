package consumer_group

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/test/scaffold"
)

var _ = ginkgo.Describe("`adc diff` consumer group tests", func() {
	ginkgo.Context("Basic functions", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should return the diff result", func() {
			out, err := s.Diff("suites-consumer-group/testdata/test.yaml")
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(out).To(gomega.Equal(`creating consumer_group: "company_a"
creating consumer: "jack"
Summary: created 2, updated 0, deleted 0
`))
		})
	})
})
