package consumer_group

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/test/cli/scaffold"
)

var _ = ginkgo.Describe("`adc dump` consumer group tests", func() {
	ginkgo.Context("Test the dump command", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should dump consumer group resources", func() {
			_, err := s.Sync("suites-consumer-group/testdata/test.yaml")
			gomega.Expect(err).To(gomega.BeNil(), "check sync command")

			out, err := s.Dump()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(out).To(gomega.Equal(`consumer_groups:
- id: company_a
  plugins:
    limit-count:
      allow_degradation: false
      count: 200
      group: $consumer_group_id
      key: remote_addr
      key_type: var
      policy: local
      rejected_code: 503
      show_limit_quota_header: true
      time_window: 60
consumers:
- plugins:
    key-auth:
      header: apikey
      hide_credentials: false
      key: auth-one
      query: apikey
  username: jack
name: ""
version: ""
`))

			err = s.DeleteConsumer("jack")
			err = s.DeleteConsumerGroup("company_a")
			gomega.Expect(err).To(gomega.BeNil(), "check consumer group delete")
		})
	})
})
