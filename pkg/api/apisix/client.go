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

	cli *http.Client
}

func newClient(baseURL, adminKey string) *Client {
	return &Client{
		baseURL:  baseURL,
		adminKey: adminKey,
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

func (c *Client) getResource(ctx context.Context, url string) (*item, error) {
	var res getResponse
	err := makeGetRequest(c, ctx, url, &res)
	if err != nil {
		return nil, err
	}
	return &res, nil
}

func (c *Client) listResource(ctx context.Context, url string) (items, error) {
	var res listResponse

	err := makeGetRequest(c, ctx, url, &res)
	if err != nil {
		return nil, err
	}
	return res.List, nil
}

func (c *Client) createResource(ctx context.Context, url string, body []byte) (*item, error) {
	var cr createResponse
	err := makePutRequest(c, ctx, url, body, &cr)
	if err != nil {
		return nil, err
	}
	return &cr, nil
}

func (c *Client) updateResource(ctx context.Context, url string, body []byte) (*item, error) {
	var ur updateResponse

	err := makePutRequest(c, ctx, url, body, &ur)
	if err != nil {
		return nil, err
	}
	return &ur, nil
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
		return handleErrorResponse(resp)
	}
	return nil
}

func readBody(r io.ReadCloser) (string, error) {
	defer r.Close()

	data, err := io.ReadAll(r)
	if err != nil {
		return "", err
	}
	return string(data), nil
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

func (c *Client) validate(ctx context.Context, url string, resource interface{}) error {
	jsonData, err := json.Marshal(resource)
	if err != nil {
		return err
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(jsonData))
	if err != nil {
		return err
	}
	resp, err := c.do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return handleErrorResponse(resp)
	}
	return nil
}

func isFunctionDisabled(msg string) bool {
	return strings.Contains(msg, "is disabled")
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
		if resp.StatusCode == http.StatusNotFound {
			return ErrNotFound
		}
		return handleErrorResponse(resp)
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
		return handleErrorResponse(resp)
	}
	dec := json.NewDecoder(resp.Body)
	if err := dec.Decode(result); err != nil {
		return err
	}

	return nil
}

func handleErrorResponse(resp *http.Response) error {
	respData := &struct {
		ErrMsg string `json:"error_msg"`
	}{}

	body, err := readBody(resp.Body)
	if err != nil {
		err = multierr.Append(err, fmt.Errorf("read body failed"))
		return err
	}
	err = json.Unmarshal([]byte(body), respData)
	if err != nil {
		return err
	}

	errMsg := errors.New(respData.ErrMsg)
	if isFunctionDisabled(errMsg.Error()) {
		return errMsg
	}
	return multierr.Append(fmt.Errorf("unexpected status code %d", resp.StatusCode), errMsg)
}
