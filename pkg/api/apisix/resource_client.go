package apisix

import (
	"context"
	"encoding/json"
)

type resourceClient[T any] struct {
	url    string
	client *Client
}

func newResourceClient[T any](c *Client, resourceName string) *resourceClient[T] {
	return &resourceClient[T]{
		url:    c.baseURL + "/" + resourceName,
		client: c,
	}
}

func (u *resourceClient[T]) Get(ctx context.Context, name string) (*T, error) {
	url := u.url + "/" + name
	resp, err := u.client.getResource(ctx, url)
	if err != nil {
		return nil, err
	}

	ups, err := unmarshalItem[T](resp)
	if err != nil {
		return nil, err
	}
	return ups, nil
}

func (u *resourceClient[T]) List(ctx context.Context) ([]*T, error) {
	svcItems, err := u.client.listResource(ctx, u.url)
	if err != nil {
		return nil, err
	}

	var items []*T
	for _, item := range svcItems {
		svc, err := unmarshalItem[T](&item)
		if err != nil {
			return nil, err
		}
		items = append(items, svc)
	}
	return items, nil
}

func (u *resourceClient[T]) Create(ctx context.Context, id string, obj *T) (*T, error) {
	body, err := json.Marshal(obj)
	if err != nil {
		return nil, err
	}
	url := u.url + "/" + id

	resp, err := u.client.createResource(ctx, url, body)
	if err != nil {
		return nil, err
	}
	svc, err := unmarshalItem[T](resp)
	if err != nil {
		return nil, err
	}
	return svc, err
}

func (u *resourceClient[T]) Delete(ctx context.Context, name string) error {
	url := u.url + "/" + name
	if err := u.client.deleteResource(ctx, url); err != nil {
		return err
	}
	return nil
}

func (u *resourceClient[T]) Update(ctx context.Context, id string, obj *T) (*T, error) {
	body, err := json.Marshal(obj)
	if err != nil {
		return nil, err
	}

	url := u.url + "/" + id
	resp, err := u.client.updateResource(ctx, url, body)
	if err != nil {
		return nil, err
	}
	svc, err := unmarshalItem[T](resp)
	if err != nil {
		return nil, err
	}
	return svc, err
}
