package consumer

import (
	"bytes"
	"os/exec"

	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/test/cli/scaffold"
)

var _ = ginkgo.Describe("`adc dump` consumer tests", func() {
	ginkgo.Context("Test the dump command", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("should dump consumer resources", func() {
			var syncOutput bytes.Buffer
			cmd := exec.Command("adc", "sync", "-f", "suites-consumer/testdata/test.yaml")
			cmd.Stdout = &syncOutput
			err := cmd.Run()
			gomega.Expect(err).To(gomega.BeNil())

			var out bytes.Buffer
			cmd = exec.Command("adc", "dump", "-o", "/dev/stdout")
			cmd.Stdout = &out
			err = cmd.Run()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(out.String()).To(gomega.Equal(`consumers:
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
