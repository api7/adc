package plugin_config

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/test/cli/scaffold"
)

var _ = ginkgo.Describe("`adc validate` plugin config tests", func() {
	ginkgo.Context("Basic functions", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should validate plugin config schema", func() {
			validateOutput, err := s.Validate("suites-plugin-config/testdata/test.yaml")
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(validateOutput).To(gomega.Equal("Read configuration file successfully: config name: , version: , plugin_configs: 1.\nValidate file content success\n"))
		})
	})
})
