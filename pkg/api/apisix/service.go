package apisix

import (
	"context"
	"encoding/json"

	"github.com/api7/adc/pkg/api/apisix/types"
)

type serviceClient struct {
	url     string
	cluster *Client
}

func newService(c *Client) Service {
	return &serviceClient{
		url:     c.baseURL + "/services",
		cluster: c,
	}
}

func (u *serviceClient) Get(ctx context.Context, name string) (*types.Service, error) {
	url := u.url + "/" + name
	resp, err := u.cluster.getResource(ctx, url)
	if err != nil {
		return nil, err
	}

	ups, err := resp.service()
	if err != nil {
		return nil, err
	}
	return ups, nil
}

// List is only used in cache warming up. So here just pass through
// to APISIX.
func (u *serviceClient) List(ctx context.Context) ([]*types.Service, error) {
	svcItems, err := u.cluster.listResource(ctx, u.url)
	if err != nil {
		return nil, err
	}

	var items []*types.Service
	for _, item := range svcItems {
		svc, err := item.service()
		if err != nil {
			return nil, err
		}
		items = append(items, svc)
	}
	return items, nil
}

func (u *serviceClient) Create(ctx context.Context, obj *types.Service) (*types.Service, error) {
	body, err := json.Marshal(obj)
	if err != nil {
		return nil, err
	}
	url := u.url + "/" + obj.Name

	resp, err := u.cluster.createResource(ctx, url, body)
	if err != nil {
		return nil, err
	}
	svc, err := resp.service()
	if err != nil {
		return nil, err
	}
	return svc, err
}

func (u *serviceClient) Delete(ctx context.Context, name string) error {
	url := u.url + "/" + name
	if err := u.cluster.deleteResource(ctx, url); err != nil {
		return err
	}
	return nil
}

func (u *serviceClient) Update(ctx context.Context, obj *types.Service) (*types.Service, error) {
	body, err := json.Marshal(obj)
	if err != nil {
		return nil, err
	}

	url := u.url + "/" + obj.Name
	resp, err := u.cluster.updateResource(ctx, url, body)
	if err != nil {
		return nil, err
	}
	svc, err := resp.service()
	if err != nil {
		return nil, err
	}
	return svc, err
}
