package suites

import (
	"context"
	"os/exec"
	"strings"

	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/pkg/api/apisix"
)

type Scaffold struct {
	cluster apisix.Cluster
}

func NewScaffold() *Scaffold {
	ginkgo.BeforeEach(func() {
		cmd := exec.Command("adc", "configure")
		cmd.Stdin = strings.NewReader("http://127.0.0.1:9180\nedd1c9f034335f136f87ad84b625c8f1\n")
		err := cmd.Run()
		gomega.Expect(err).To(gomega.BeNil())
	})

	ginkgo.AfterEach(func() {
		// TODO: delete all created resources
	})

	return &Scaffold{
		cluster: apisix.NewCluster(context.Background(), "http://127.0.0.1:9180", "edd1c9f034335f136f87ad84b625c8f1"),
	}
}

func (s *Scaffold) DeleteService(id string) error {
	return s.cluster.Service().Delete(context.Background(), id)
}

func (s *Scaffold) DeleteRoute(id string) error {
	return s.cluster.Route().Delete(context.Background(), id)
}
