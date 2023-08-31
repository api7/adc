package scaffold

import (
	"context"
	"os/exec"
	"strings"

	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/pkg/api/apisix"
	"github.com/api7/adc/pkg/api/apisix/types"
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

func (s *Scaffold) GetRoute(route string) (*types.Route, error) {
	return s.cluster.Route().Get(context.Background(), route)
}

func (s *Scaffold) ListRoute() ([]*types.Route, error) {
	return s.cluster.Route().List(context.Background())
}

func (s *Scaffold) CreateRoute(route *types.Route) (*types.Route, error) {
	return s.cluster.Route().Create(context.Background(), route)
}

func (s *Scaffold) UpdateRoute(route *types.Route) (*types.Route, error) {
	return s.cluster.Route().Update(context.Background(), route)
}

func (s *Scaffold) DeleteRoute(id string) error {
	return s.cluster.Route().Delete(context.Background(), id)
}

func (s *Scaffold) GetService(service string) (*types.Service, error) {
	return s.cluster.Service().Get(context.Background(), service)
}

func (s *Scaffold) ListService() ([]*types.Service, error) {
	return s.cluster.Service().List(context.Background())
}

func (s *Scaffold) CreateService(service *types.Service) (*types.Service, error) {
	return s.cluster.Service().Create(context.Background(), service)
}

func (s *Scaffold) UpdateService(service *types.Service) (*types.Service, error) {
	return s.cluster.Service().Update(context.Background(), service)
}

func (s *Scaffold) DeleteService(id string) error {
	return s.cluster.Service().Delete(context.Background(), id)
}

func (s *Scaffold) GetConsumer(service string) (*types.Consumer, error) {
	return s.cluster.Consumer().Get(context.Background(), service)
}

func (s *Scaffold) ListConsumer() ([]*types.Consumer, error) {
	return s.cluster.Consumer().List(context.Background())
}

func (s *Scaffold) CreateConsumer(service *types.Consumer) (*types.Consumer, error) {
	return s.cluster.Consumer().Create(context.Background(), service)
}

func (s *Scaffold) UpdateConsumer(service *types.Consumer) (*types.Consumer, error) {
	return s.cluster.Consumer().Update(context.Background(), service)
}

func (s *Scaffold) DeleteConsumer(id string) error {
	return s.cluster.Consumer().Delete(context.Background(), id)
}
