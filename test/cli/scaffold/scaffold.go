package scaffold

import (
	"bytes"
	"context"
	"os/exec"
	"strings"

	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/pkg/api/apisix"
	"github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/pkg/common"
)

type Scaffold struct {
	cluster apisix.Cluster

	routes         map[string]struct{}
	services       map[string]struct{}
	consumers      map[string]struct{}
	globalRules    map[string]struct{}
	pluginConfigs  map[string]struct{}
	consumerGroups map[string]struct{}
}

func NewScaffold() *Scaffold {
	s := &Scaffold{
		cluster: apisix.NewCluster(context.Background(), "http://127.0.0.1:9180", "edd1c9f034335f136f87ad84b625c8f1"),

		routes:         map[string]struct{}{},
		services:       map[string]struct{}{},
		consumers:      map[string]struct{}{},
		globalRules:    map[string]struct{}{},
		pluginConfigs:  map[string]struct{}{},
		consumerGroups: map[string]struct{}{},
	}

	ginkgo.BeforeEach(func() {
		err := s.Configure("http://127.0.0.1:9180", "edd1c9f034335f136f87ad84b625c8f1")
		gomega.Expect(err).To(gomega.BeNil())
	})

	ginkgo.AfterEach(func() {
		for route, _ := range s.routes {
			s.DeleteRoute(route)
		}
		for svc, _ := range s.services {
			s.DeleteService(svc)
		}
		for consumer, _ := range s.consumers {
			s.DeleteConsumer(consumer)
		}
		for globalRule, _ := range s.globalRules {
			s.DeleteGlobalRule(globalRule)
		}
		for pluginConfig, _ := range s.pluginConfigs {
			s.DeletePluginConfig(pluginConfig)
		}
		for consumerGroup, _ := range s.consumerGroups {
			s.DeleteConsumerGroup(consumerGroup)
		}
	})

	return s
}

func (s *Scaffold) AddRoutesFinalizer(routes ...string) {
	for _, route := range routes {
		s.routes[route] = struct{}{}
	}
}

func (s *Scaffold) AddServicesFinalizer(services ...string) {
	for _, service := range services {
		s.services[service] = struct{}{}
	}
}

func (s *Scaffold) AddConsumersFinalizer(consumers ...string) {
	for _, consumer := range consumers {
		s.consumers[consumer] = struct{}{}
	}
}

func (s *Scaffold) AddGlobalRulesFinalizer(globalRules ...string) {
	for _, globalRule := range globalRules {
		s.globalRules[globalRule] = struct{}{}
	}
}

func (s *Scaffold) AddPluginConfigsFinalizer(pluginConfigs ...string) {
	for _, pluginConfig := range pluginConfigs {
		s.pluginConfigs[pluginConfig] = struct{}{}
	}
}

func (s *Scaffold) AddConsumerGroupsFinalizer(consumerGroups ...string) {
	for _, consumerGroup := range consumerGroups {
		s.consumerGroups[consumerGroup] = struct{}{}
	}
}

func (s *Scaffold) Configure(host, key string) error {
	input := host + "\n" + key + "\n"
	_, err := s.ExecWithInput(input, "configure")
	return err
}

func (s *Scaffold) Exec(args ...string) (string, error) {
	var syncOutput bytes.Buffer
	cmd := exec.Command("adc", args...)
	cmd.Stdout = &syncOutput
	err := cmd.Run()

	return syncOutput.String(), err
}

func (s *Scaffold) ExecWithInput(input string, args ...string) (string, error) {
	var syncOutput bytes.Buffer
	cmd := exec.Command("adc", args...)
	cmd.Stdout = &syncOutput
	cmd.Stdin = strings.NewReader(input)
	err := cmd.Run()

	return syncOutput.String(), err
}

func (s *Scaffold) Ping() (string, error) {
	return s.Exec("ping")
}

func (s *Scaffold) Sync(path string) (string, error) {
	conf, err := common.GetContentFromFile(path)
	gomega.Expect(err).To(gomega.BeNil(), "load config from file "+path)
	for _, route := range conf.Routes {
		s.AddRoutesFinalizer(route.ID)
	}

	for _, service := range conf.Services {
		s.AddServicesFinalizer(service.ID)
	}

	for _, consumer := range conf.Consumers {
		s.AddConsumersFinalizer(consumer.Username)
	}

	for _, globalRule := range conf.GlobalRules {
		s.AddGlobalRulesFinalizer(globalRule.ID)
	}

	for _, pluginConfig := range conf.PluginConfigs {
		s.AddPluginConfigsFinalizer(pluginConfig.ID)
	}

	for _, consumerGroup := range conf.ConsumerGroups {
		s.AddConsumerGroupsFinalizer(consumerGroup.ID)
	}

	return s.Exec("sync", "-f", path)
}

func (s *Scaffold) Dump() (string, error) {
	return s.Exec("dump", "-o", "/dev/stdout")
}

func (s *Scaffold) Diff(path string) (string, error) {
	return s.Exec("diff", "-f", path)
}

func (s *Scaffold) Validate(path string) (string, error) {
	return s.Exec("validate", "-f", path)
}

func (s *Scaffold) GetRoute(route string) (*types.Route, error) {
	return s.cluster.Route().Get(context.Background(), route)
}

func (s *Scaffold) ListRoute() ([]*types.Route, error) {
	return s.cluster.Route().List(context.Background())
}

func (s *Scaffold) CreateRoute(route *types.Route) (*types.Route, error) {
	s.routes[route.ID] = struct{}{}

	return s.cluster.Route().Create(context.Background(), route)
}

func (s *Scaffold) UpdateRoute(route *types.Route) (*types.Route, error) {
	s.routes[route.ID] = struct{}{}

	return s.cluster.Route().Update(context.Background(), route)
}

func (s *Scaffold) DeleteRoute(id string) error {
	delete(s.routes, id)

	return s.cluster.Route().Delete(context.Background(), id)
}

func (s *Scaffold) GetService(service string) (*types.Service, error) {
	return s.cluster.Service().Get(context.Background(), service)
}

func (s *Scaffold) ListService() ([]*types.Service, error) {
	return s.cluster.Service().List(context.Background())
}

func (s *Scaffold) CreateService(service *types.Service) (*types.Service, error) {
	s.services[service.ID] = struct{}{}

	return s.cluster.Service().Create(context.Background(), service)
}

func (s *Scaffold) UpdateService(service *types.Service) (*types.Service, error) {
	s.services[service.ID] = struct{}{}

	return s.cluster.Service().Update(context.Background(), service)
}

func (s *Scaffold) DeleteService(id string) error {
	delete(s.services, id)

	return s.cluster.Service().Delete(context.Background(), id)
}

func (s *Scaffold) GetConsumer(username string) (*types.Consumer, error) {
	return s.cluster.Consumer().Get(context.Background(), username)
}

func (s *Scaffold) ListConsumer() ([]*types.Consumer, error) {
	return s.cluster.Consumer().List(context.Background())
}

func (s *Scaffold) CreateConsumer(consumer *types.Consumer) (*types.Consumer, error) {
	s.consumers[consumer.Username] = struct{}{}

	return s.cluster.Consumer().Create(context.Background(), consumer)
}

func (s *Scaffold) UpdateConsumer(consumer *types.Consumer) (*types.Consumer, error) {
	s.consumers[consumer.Username] = struct{}{}

	return s.cluster.Consumer().Update(context.Background(), consumer)
}

func (s *Scaffold) DeleteConsumer(id string) error {
	delete(s.consumers, id)

	return s.cluster.Consumer().Delete(context.Background(), id)
}

func (s *Scaffold) GetGlobalRule(id string) (*types.GlobalRule, error) {
	return s.cluster.GlobalRule().Get(context.Background(), id)
}

func (s *Scaffold) ListGlobalRule() ([]*types.GlobalRule, error) {
	return s.cluster.GlobalRule().List(context.Background())
}

func (s *Scaffold) CreateGlobalRule(globalRule *types.GlobalRule) (*types.GlobalRule, error) {
	s.globalRules[globalRule.ID] = struct{}{}

	return s.cluster.GlobalRule().Create(context.Background(), globalRule)
}

func (s *Scaffold) UpdateGlobalRule(globalRule *types.GlobalRule) (*types.GlobalRule, error) {
	s.globalRules[globalRule.ID] = struct{}{}

	return s.cluster.GlobalRule().Update(context.Background(), globalRule)
}

func (s *Scaffold) DeleteGlobalRule(id string) error {
	delete(s.globalRules, id)

	return s.cluster.GlobalRule().Delete(context.Background(), id)
}

func (s *Scaffold) GetPluginConfig(id string) (*types.PluginConfig, error) {
	return s.cluster.PluginConfig().Get(context.Background(), id)
}

func (s *Scaffold) ListPluginConfig() ([]*types.PluginConfig, error) {
	return s.cluster.PluginConfig().List(context.Background())
}

func (s *Scaffold) CreatePluginConfig(pluginConfig *types.PluginConfig) (*types.PluginConfig, error) {
	s.pluginConfigs[pluginConfig.ID] = struct{}{}

	return s.cluster.PluginConfig().Create(context.Background(), pluginConfig)
}

func (s *Scaffold) UpdatePluginConfig(pluginConfig *types.PluginConfig) (*types.PluginConfig, error) {
	s.pluginConfigs[pluginConfig.ID] = struct{}{}

	return s.cluster.PluginConfig().Update(context.Background(), pluginConfig)
}

func (s *Scaffold) DeletePluginConfig(id string) error {
	delete(s.pluginConfigs, id)

	return s.cluster.PluginConfig().Delete(context.Background(), id)
}

func (s *Scaffold) GetConsumerGroup(id string) (*types.ConsumerGroup, error) {
	return s.cluster.ConsumerGroup().Get(context.Background(), id)
}

func (s *Scaffold) ListConsumerGroup() ([]*types.ConsumerGroup, error) {
	return s.cluster.ConsumerGroup().List(context.Background())
}

func (s *Scaffold) CreateConsumerGroup(consumerGroup *types.ConsumerGroup) (*types.ConsumerGroup, error) {
	s.consumerGroups[consumerGroup.ID] = struct{}{}

	return s.cluster.ConsumerGroup().Create(context.Background(), consumerGroup)
}

func (s *Scaffold) UpdateConsumerGroup(consumerGroup *types.ConsumerGroup) (*types.ConsumerGroup, error) {
	s.consumerGroups[consumerGroup.ID] = struct{}{}

	return s.cluster.ConsumerGroup().Update(context.Background(), consumerGroup)
}

func (s *Scaffold) DeleteConsumerGroup(id string) error {
	delete(s.consumerGroups, id)

	return s.cluster.ConsumerGroup().Delete(context.Background(), id)
}
