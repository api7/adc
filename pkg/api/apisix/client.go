package apisix

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"

	"go.uber.org/multierr"
)

var (
	ErrNotFound         = fmt.Errorf("not found")
	ErrStillInUse       = errors.New("still in use") // We should use force mode
	ErrFunctionDisabled = errors.New("function disabled")
)

type Client struct {
	baseURL  string
	adminKey string

	adminVersion string
	cli          *http.Client
}

func newClient(baseURL, adminKey, version string) *Client {
	return &Client{
		baseURL:      baseURL,
		adminKey:     adminKey,
		adminVersion: version,
		cli: &http.Client{
			Timeout: 5 * time.Second,
		},
	}
}

func (c *Client) setAdminKey(req *http.Request) {
	if c.adminKey != "" {
		req.Header.Set("X-API-Key", c.adminKey)
	}
}

func (c *Client) do(req *http.Request) (*http.Response, error) {
	c.setAdminKey(req)
	return c.cli.Do(req)
}

func (c *Client) isFunctionDisabled(body string) bool {
	return strings.Contains(body, "is disabled")
}

func (c *Client) getResource(ctx context.Context, url string) (*item, error) {
	if c.adminVersion == "v3" {
		var res item
		err := makeGetRequest(c, ctx, url, &res)
		if err != nil {
			return nil, err
		}
		return &res, nil
	}

	var res getResponse
	err := makeGetRequest(c, ctx, url, &res)
	if err != nil {
		return nil, err
	}
	return &res.Item, nil
}

func (c *Client) listResource(ctx context.Context, url string) (items, error) {
	if c.adminVersion == "v3" {
		var res listResponseV3

		err := makeGetRequest(c, ctx, url, &res)
		if err != nil {
			return nil, err
		}
		return res.List, nil
	}

	var res listResponse
	err := makeGetRequest(c, ctx, url, &res)
	if err != nil {
		return nil, err
	}
	return res.Node.Items, nil
}

func (c *Client) createResource(ctx context.Context, url string, body []byte) (*item, error) {
	if c.adminVersion == "v3" {
		var cr createResponseV3
		err := makePutRequest(c, ctx, url, body, &cr)
		if err != nil {
			return nil, err
		}
		return &cr.item, nil
	}

	var cr createResponse
	err := makePutRequest(c, ctx, url, body, &cr)
	if err != nil {
		return nil, err
	}
	return &cr.Item, nil
}

func (c *Client) updateResource(ctx context.Context, url string, body []byte) (*item, error) {
	if c.adminVersion == "v3" {
		var ur updateResponseV3

		err := makePutRequest(c, ctx, url, body, &ur)
		if err != nil {
			return nil, err
		}
		return &ur.item, nil
	}
	var ur updateResponse
	err := makePutRequest(c, ctx, url, body, &ur)
	if err != nil {
		return nil, err
	}
	return &ur.Item, nil
}

func (c *Client) deleteResource(ctx context.Context, url string) error {
	req, err := http.NewRequestWithContext(ctx, http.MethodDelete, url, nil)
	if err != nil {
		return err
	}
	resp, err := c.do(req)
	if err != nil {
		return err
	}

	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK && resp.StatusCode != http.StatusNoContent && resp.StatusCode != http.StatusNotFound {
		message := readBody(resp.Body)
		if c.isFunctionDisabled(message) {
			return ErrFunctionDisabled
		}
		err = multierr.Append(err, fmt.Errorf("unexpected status code %d", resp.StatusCode))
		err = multierr.Append(err, fmt.Errorf("error message: %s", message))
		if strings.Contains(message, "still using") {
			return ErrStillInUse
		}
		return err
	}
	return nil
}

func readBody(r io.ReadCloser) string {
	defer r.Close()

	data, err := io.ReadAll(r)
	if err != nil {
		return ""
	}
	return string(data)
}

// getSchema returns the schema of APISIX object.
func (c *Client) getSchema(ctx context.Context, url string) (string, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return "", err
	}
	resp, err := c.do(req)
	if err != nil {
		return "", err
	}

	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		if resp.StatusCode == http.StatusNotFound {
			return "", ErrNotFound
		} else {
			err = multierr.Append(err, fmt.Errorf("unexpected status code %d", resp.StatusCode))
			err = multierr.Append(err, fmt.Errorf("error message: %s", readBody(resp.Body)))
		}
		return "", err
	}

	return readBody(resp.Body), nil
}

// getList returns a list of string.
func (c *Client) getList(ctx context.Context, url string) ([]string, error) {
	var listResp map[string]interface{}
	err := makeGetRequest(c, ctx, url, &listResp)
	if err != nil {
		return nil, err
	}
	res := make([]string, 0, len(listResp))

	for name := range listResp {
		res = append(res, name)
	}
	return res, nil
}

func makeGetRequest[T any](c *Client, ctx context.Context, url string, result *T) error {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return err
	}
	resp, err := c.do(req)
	if err != nil {
		return err
	}

	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		body := readBody(resp.Body)
		if c.isFunctionDisabled(body) {
			return ErrFunctionDisabled
		}
		err = multierr.Append(err, fmt.Errorf("unexpected status code %d", resp.StatusCode))
		err = multierr.Append(err, fmt.Errorf("error message: %s", body))
		return err
	}

	dec := json.NewDecoder(resp.Body)
	if err := dec.Decode(result); err != nil {
		return err
	}

	return nil
}

func makePutRequest[T any](c *Client, ctx context.Context, url string, body []byte, result *T) error {
	req, err := http.NewRequestWithContext(ctx, http.MethodPut, url, bytes.NewReader(body))
	if err != nil {
		return err
	}
	resp, err := c.do(req)
	if err != nil {
		return err
	}

	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK && resp.StatusCode != http.StatusCreated {
		body := readBody(resp.Body)
		if c.isFunctionDisabled(body) {
			return ErrFunctionDisabled
		}
		err = multierr.Append(err, fmt.Errorf("unexpected status code %d", resp.StatusCode))
		err = multierr.Append(err, fmt.Errorf("error message: %s", body))
		return err
	}
	dec := json.NewDecoder(resp.Body)
	if err := dec.Decode(result); err != nil {
		return err
	}

	return nil
}
