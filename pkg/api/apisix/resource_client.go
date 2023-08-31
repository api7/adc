package apisix

import (
	"context"
	"encoding/json"
	"fmt"
	"reflect"
	"strings"
)

type resourceClient[T any] struct {
	baseURL      string
	resourceName string
	resourceURL  string
	validateURL  string
	client       *Client
}

func newResourceClient[T any](c *Client, resourceName string) *resourceClient[T] {
	baseURL := c.baseURL
	if !strings.HasSuffix(baseURL, "/") {
		baseURL += "/"
	}
	if !strings.HasSuffix(baseURL, "apisix/admin/") {
		baseURL += "apisix/admin/"
	}

	return &resourceClient[T]{
		baseURL:      baseURL,
		resourceName: resourceName,
		resourceURL:  baseURL + resourceName,
		validateURL:  baseURL + "schema/validate/" + resourceName,
		client:       c,
	}
}

func (u *resourceClient[T]) Get(ctx context.Context, name string) (*T, error) {
	url := u.resourceURL + "/" + name
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
	svcItems, err := u.client.listResource(ctx, u.resourceURL)
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
	url := u.resourceURL + "/" + id

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
	url := u.resourceURL + "/" + name
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

	url := u.resourceURL + "/" + id
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

func GetResourceNameOrID(resource interface{}) string {
	value := reflect.ValueOf(resource)
	value = reflect.Indirect(value)
	nameOrID := value.FieldByName("ID")
	if !nameOrID.IsValid() {
		nameOrID = value.FieldByName("Name")
	}
	if !nameOrID.IsValid() {
		nameOrID = value.FieldByName("Username")
	}
	return nameOrID.String()
}

func (u *resourceClient[T]) Validate(ctx context.Context, resource *T) error {
	err := u.client.validate(ctx, u.validateURL, resource)
	if err != nil {
		return fmt.Errorf("failed to validate resource '%s (%s)': %s", u.resourceName, GetResourceNameOrID(resource), err.Error())
	}
	return nil
}
