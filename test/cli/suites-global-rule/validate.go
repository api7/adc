package global_rule

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/test/cli/scaffold"
)

var _ = ginkgo.Describe("`adc validate` global rule tests", func() {
	ginkgo.Context("Basic functions", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should validate global rule schema", func() {
			validateOutput, err := s.Validate("suites-global-rule/testdata/test.yaml")
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(validateOutput).To(gomega.Equal("Get file content success: config name: , version: , global_rules: 1.\nValidate file content success\n"))
		})
	})
})
