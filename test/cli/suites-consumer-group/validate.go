package consumer_group

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/test/cli/scaffold"
)

var _ = ginkgo.Describe("`adc validate` consumer group tests", func() {
	ginkgo.Context("Basic functions", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should validate consumer group schema", func() {
			validateOutput, err := s.Validate("suites-consumer-group/testdata/test.yaml")
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(validateOutput).To(gomega.Equal("Read configuration file successfully: config name: , version: , consumers: 1, consumer_groups: 1.\nValidate file content success\n"))
		})
	})
})
