package apisix

import (
	"context"
	"encoding/json"

	"github.com/api7/adc/pkg/api/apisix/types"
)

type upstreamClient struct {
	url     string
	cluster *Client
}

func newUpstream(c *Client) Upstream {
	return &upstreamClient{
		url:     c.baseURL + "/upstreams",
		cluster: c,
	}
}

func (u *upstreamClient) Get(ctx context.Context, name string) (*types.Upstream, error) {
	url := u.url + "/" + name
	resp, err := u.cluster.getResource(ctx, url)
	if err != nil {
		return nil, err
	}

	ups, err := resp.upstream()
	if err != nil {
		return nil, err
	}
	return ups, nil
}

// List is only used in cache warming up. So here just pass through
// to APISIX.
func (u *upstreamClient) List(ctx context.Context) ([]*types.Upstream, error) {
	upsItems, err := u.cluster.listResource(ctx, u.url)
	if err != nil {
		return nil, err
	}

	var items []*types.Upstream
	for _, item := range upsItems {
		ups, err := item.upstream()
		if err != nil {
			return nil, err
		}
		items = append(items, ups)
	}
	return items, nil
}

func (u *upstreamClient) Create(ctx context.Context, obj *types.Upstream) (*types.Upstream, error) {
	body, err := json.Marshal(obj)
	if err != nil {
		return nil, err
	}
	url := u.url + "/" + obj.Name

	resp, err := u.cluster.createResource(ctx, url, body)
	if err != nil {
		return nil, err
	}
	ups, err := resp.upstream()
	if err != nil {
		return nil, err
	}
	return ups, err
}

func (u *upstreamClient) Delete(ctx context.Context, name string) error {
	url := u.url + "/" + name
	if err := u.cluster.deleteResource(ctx, url); err != nil {
		return err
	}
	return nil
}

func (u *upstreamClient) Update(ctx context.Context, obj *types.Upstream) (*types.Upstream, error) {
	body, err := json.Marshal(obj)
	if err != nil {
		return nil, err
	}

	url := u.url + "/" + obj.Name
	resp, err := u.cluster.updateResource(ctx, url, body)
	if err != nil {
		return nil, err
	}
	ups, err := resp.upstream()
	if err != nil {
		return nil, err
	}
	return ups, err
}
