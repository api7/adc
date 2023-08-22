package apisix

import (
	"context"
	"encoding/json"

	"github.com/api7/adc/pkg/api/apisix/types"
)

type upstreamClient struct {
	url    string
	client *Client
}

func newUpstream(c *Client) Upstream {
	return &upstreamClient{
		url:    c.baseURL + "/upstreams",
		client: c,
	}
}

func (u *upstreamClient) Get(ctx context.Context, name string) (*types.Upstream, error) {
	url := u.url + "/" + name
	resp, err := u.client.getResource(ctx, url)
	if err != nil {
		return nil, err
	}

	ups, err := resp.upstream()
	if err != nil {
		return nil, err
	}
	return ups, nil
}

func (u *upstreamClient) List(ctx context.Context) ([]*types.Upstream, error) {
	upsItems, err := u.client.listResource(ctx, u.url)
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

	resp, err := u.client.createResource(ctx, url, body)
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
	if err := u.client.deleteResource(ctx, url); err != nil {
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
	resp, err := u.client.updateResource(ctx, url, body)
	if err != nil {
		return nil, err
	}
	ups, err := resp.upstream()
	if err != nil {
		return nil, err
	}
	return ups, err
}
