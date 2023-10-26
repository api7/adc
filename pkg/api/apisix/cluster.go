package apisix

import (
	"context"
	"crypto/tls"
	"crypto/x509"
	"errors"
	"net/url"
	"os"
	"strings"

	"github.com/fatih/color"

	"github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/pkg/config"
)

type cluster struct {
	baseURL  string
	adminKey string

	cli *Client

	route          Route
	service        Service
	consumer       Consumer
	ssl            SSL
	globalRule     GlobalRule
	pluginConfig   PluginConfig
	consumerGroup  ConsumerGroup
	pluginMetadata PluginMetadata
	streamRoute    StreamRoute
	upstream       Upstream
}

func NewCluster(ctx context.Context, conf config.ClientConfig) (Cluster, error) {
	c := &cluster{
		baseURL:  conf.Server,
		adminKey: conf.Token,
	}

	var cli *Client
	if conf.CAPath != "" && conf.Certificate != "" && conf.CertificateKey != "" {
		rootCA, err := os.ReadFile(conf.CAPath)
		if err != nil {
			color.Red("Failed to read CA file: %v", err)
			return nil, err
		}

		caCertPool := x509.NewCertPool()
		ok := caCertPool.AppendCertsFromPEM(rootCA)
		if !ok {
			color.Red("Failed to parse CA certificate")
			return nil, errors.New("failed to parse CA certificate")
		}

		cert, err := os.ReadFile(conf.Certificate)
		if err != nil {
			color.Red("Failed to read certificate file: %v", err)
			return nil, err
		}
		key, err := os.ReadFile(conf.CertificateKey)
		if err != nil {
			color.Red("Failed to read certificate key file: %v", err)
			return nil, err
		}
		keyPair, err := tls.X509KeyPair(cert, key)
		if err != nil {
			color.Red("Failed to parse x509 key pair: %v", err)
			return nil, err
		}

		u, err := url.Parse(conf.Server)
		if err != nil {
			color.Red("Failed to parse APISIX address: %v", err)
		}

		cli = newClientWithCertificates(c.baseURL, c.adminKey, u.Hostname(), conf.Insecure, caCertPool, []tls.Certificate{keyPair})
	} else {
		cli = newClient(c.baseURL, c.adminKey)
	}

	c.cli = cli
	c.route = newRoute(cli)
	c.service = newService(cli)
	c.consumer = newConsumer(cli)
	c.ssl = newSSL(cli)
	c.globalRule = newGlobalRule(cli)
	c.pluginConfig = newPluginConfig(cli)
	c.consumerGroup = newConsumerGroup(cli)
	c.pluginMetadata = newPluginMetadata(cli)
	c.streamRoute = newStreamRoute(cli)
	c.upstream = newUpstream(cli)

	return c, nil
}

// Route implements Cluster.Route method.
func (c *cluster) Route() Route {
	return c.route
}

// Service implements Cluster.Service method.
func (c *cluster) Service() Service {
	return c.service
}

// Consumer implements Cluster.Consumer method.
func (c *cluster) Consumer() Consumer {
	return c.consumer
}

// SSL implements ClusterSSL method.
func (c *cluster) SSL() SSL {
	return c.ssl
}

// GlobalRule implements Cluster.GlobalRule method.
func (c *cluster) GlobalRule() GlobalRule {
	return c.globalRule
}

// PluginConfig implements Cluster.PluginConfig method.
func (c *cluster) PluginConfig() PluginConfig {
	return c.pluginConfig
}

// ConsumerGroup implements Cluster.ConsumerGroup method.
func (c *cluster) ConsumerGroup() ConsumerGroup {
	return c.consumerGroup
}

// PluginMetadata implements Cluster.PluginMetadata method.
func (c *cluster) PluginMetadata() PluginMetadata {
	return c.pluginMetadata
}

// StreamRoute implements Cluster.StreamRoute method.
func (c *cluster) StreamRoute() StreamRoute {
	return c.streamRoute
}

// Upstream implements Cluster.Upstream method.
func (c *cluster) Upstream() Upstream {
	return c.upstream
}

func (c *cluster) Ping() error {
	_, err := c.Route().List(context.Background())
	return err
}

func (c *cluster) SupportValidate() (bool, error) {
	err := c.Route().Validate(context.Background(), &types.Route{
		ID:   "test_validate_api",
		Name: "test_validate_api",
		Host: "httpbin.org",
		Uri:  "/*",
		Upstream: &types.Upstream{
			ID:   "test_validate_api",
			Name: "test_validate_api",
			Type: "roundrobin",
			Nodes: []types.UpstreamNode{
				{
					Host:   "httpbin.org",
					Port:   80,
					Weight: 1,
				},
			},
		},
	})
	if err == nil {
		return true, nil
	}
	if strings.Contains(err.Error(), "not found") {
		return false, nil
	}

	return false, err
}

func (c *cluster) SupportStreamRoute() (bool, error) {
	cli := newResourceClient[types.StreamRoute](c.cli, "stream_routes")
	_, err := cli.List(context.Background())
	if err != nil {
		if strings.Contains(err.Error(), "stream mode is disabled") {
			return false, nil
		}
		return false, err
	}
	return true, nil
}
