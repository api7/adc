package consumer

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/test/cli/scaffold"
)

var _ = ginkgo.Describe("`adc dump` consumer tests", func() {
	ginkgo.Context("Test the dump command", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should dump consumer resources", func() {
			_, err := s.Sync("suites-consumer/testdata/test.yaml")
			gomega.Expect(err).To(gomega.BeNil(), "check sync command")

			out, err := s.Dump()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(out).To(gomega.Equal(`consumers:
- plugins:
    key-auth:
      key: auth-one
    limit-count:
      allow_degradation: false
      count: 2
      key: remote_addr
      key_type: var
      policy: local
      rejected_code: 503
      show_limit_quota_header: true
      time_window: 60
  username: jack
name: ""
version: ""
`))

			err = s.DeleteConsumer("jack")
			gomega.Expect(err).To(gomega.BeNil(), "check consumer delete")
		})
	})
})
