package suites

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/test/cli/scaffold"
)

var _ = ginkgo.Describe("`adc validate` tests", func() {
	ginkgo.Context("Basic functions", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should validate schema", func() {
			validateOutput, err := s.Validate("testdata/test.yaml")
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(validateOutput).To(gomega.Equal("Get file content success: config name: test, version: 1.0.0, routes: 2, services: 2.\nValidate file content success\n"))
		})
	})
})
