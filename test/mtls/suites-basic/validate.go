package basic

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/test/scaffold"
)

var _ = ginkgo.Describe("`adc validate` tests", func() {
	ginkgo.Context("Basic functions", func() {
		s := scaffold.NewMtlsScaffold()
		ginkgo.It("should validate schema", func() {
			validateOutput, err := s.Validate("suites-basic/testdata/test.yaml")
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(validateOutput).To(gomega.Equal("Read configuration file successfully: config name: test, version: 1.0.0, routes: 2, services: 2.\nSuccessfully validated configuration file!\n"))
		})
	})
})
