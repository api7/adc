package apisix

import (
	"context"
)

type cluster struct {
	baseURL  string
	adminKey string

	cli *Client

	route      Route
	service    Service
	consumer   Consumer
	ssl        SSL
	globalRule GlobalRule
}

func NewCluster(ctx context.Context, url, adminKey string) Cluster {
	c := &cluster{
		baseURL:  url,
		adminKey: adminKey,
	}

	cli := newClient(url, adminKey)
	c.cli = cli

	c.route = newRoute(cli)
	c.service = newService(cli)
	c.consumer = newConsumer(cli)
	c.ssl = newSSL(cli)
	c.globalRule = newGlobalRule(cli)

	return c
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
