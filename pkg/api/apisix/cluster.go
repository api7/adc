package apisix

import (
	"context"
)

type cluster struct {
	baseURL  string
	adminKey string

	cli *Client

	upstream Upstream
	service  Service
}

func NewCluster(ctx context.Context, url, adminKey string) Cluster {
	c := &cluster{
		baseURL:  url,
		adminKey: adminKey,
	}

	cli := newClient(url, adminKey)
	c.cli = cli

	c.upstream = newUpstream(cli)
	c.service = newService(cli)

	return c
}

// Upstream implements Cluster.Upstream method.
func (c *cluster) Upstream() Upstream {
	return c.upstream
}

// Service implements Cluster.Service method.
func (c *cluster) Service() Service {
	return c.service
}
