package suites

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/test/cli/scaffold"
)

var _ = ginkgo.Describe("`adc ping` tests", func() {
	ginkgo.Context("Basic functions", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should connect to APISIX", func() {
			output, err := s.Ping()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(output).To(gomega.Equal("Successfully connected to APISIX\n"))
		})
	})
})
