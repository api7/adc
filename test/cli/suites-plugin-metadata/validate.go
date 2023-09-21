package plugin_config

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/test/scaffold"
)

var _ = ginkgo.Describe("`adc validate` plugin metadata tests", func() {
	ginkgo.Context("Basic functions", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should validate plugin metadata schema", func() {
			ginkgo.Skip("APISIX doesn't support yet")
			validateOutput, err := s.Validate("suites-plugin-metadata/testdata/test.yaml")
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(validateOutput).To(gomega.Equal("Get file content success: config name: , version: , plugin_configs: 1.\nValidate file content success\n"))
		})
	})
})
