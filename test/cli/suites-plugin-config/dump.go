package plugin_config

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/test/scaffold"
)

var _ = ginkgo.Describe("`adc dump` plugin config tests", func() {
	ginkgo.Context("Test the dump command", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should dump plugin config resources", func() {
			_, err := s.Sync("suites-plugin-config/testdata/test.yaml")
			gomega.Expect(err).To(gomega.BeNil(), "check sync command")

			out, err := s.Dump()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(out).To(gomega.Equal(`name: ""
plugin_configs:
- desc: enable limit-count plugin
  id: "1"
  plugins:
    limit-count:
      allow_degradation: false
      count: 2
      key: remote_addr
      key_type: var
      policy: local
      rejected_code: 503
      show_limit_quota_header: true
      time_window: 60
version: ""
`))

			err = s.DeletePluginConfig("1")
			gomega.Expect(err).To(gomega.BeNil(), "check plugin config delete")
		})
	})
})
