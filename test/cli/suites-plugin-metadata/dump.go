package plugin_config

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/test/scaffold"
)

var _ = ginkgo.Describe("`adc dump` plugin metadata tests", func() {
	ginkgo.Context("Test the dump command", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should dump plugin metadata resources", func() {
			_, err := s.Sync("suites-plugin-metadata/testdata/test.yaml")
			gomega.Expect(err).To(gomega.BeNil(), "check sync command")

			out, err := s.Dump()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(out).To(gomega.Equal(`name: ""
plugin_metadatas:
- id: http-logger
  log_format:
    '@timestamp': $time_iso8601
    client_ip: $remote_addr
    host: $host
version: ""
`))

			err = s.DeletePluginConfig("1")
			gomega.Expect(err).To(gomega.BeNil(), "check plugin metadata delete")
		})
	})
})
