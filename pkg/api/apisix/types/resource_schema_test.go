package types

import (
	"github.com/stretchr/testify/assert"
	"sigs.k8s.io/yaml"
	"testing"
)

func ExpectInputHasOutput(t *testing.T, input string, output string) {
	output = "name: \"\"\n" + output + "version: \"\"\n"

	var content Configuration

	err := yaml.Unmarshal([]byte(input), &content)
	assert.Nil(t, err)

	o, err := yaml.Marshal(content)
	assert.Nil(t, err)

	assert.Equal(t, output, string(o))
}

func TestRouteWithUpstreamDefaultValues(t *testing.T) {
	input := `routes:
  - name: route1
    uri: "/get"
    methods:
      - GET
      - PUT
    upstream:
      id: httpbin
      name: httpbin
      nodes:
        - host: HTTPBIN_PLACEHOLDER
          port: 80
          weight: 1
`

	output := `routes:
- id: ""
  methods:
  - GET
  - PUT
  name: route1
  priority: 0
  status: 1
  upstream:
    hash_on: vars
    id: httpbin
    name: httpbin
    nodes:
    - host: HTTPBIN_PLACEHOLDER
      port: 80
      priority: 0
      weight: 1
    pass_host: pass
    scheme: http
    type: roundrobin
  uri: /get
`

	ExpectInputHasOutput(t, input, output)
}

func TestStreamRouteWithUpstreamDefaultValues(t *testing.T) {
	input := `stream_routes:
  - id: "1"
    server_port: 9100
    upstream:
      id: httpbin
      name: httpbin
      nodes:
        - host: HTTPBIN_PLACEHOLDER
          port: 80
          weight: 1
`

	output := `stream_routes:
- id: "1"
  server_port: 9100
  upstream:
    hash_on: vars
    id: httpbin
    name: httpbin
    nodes:
    - host: HTTPBIN_PLACEHOLDER
      port: 80
      priority: 0
      weight: 1
    pass_host: pass
    scheme: tcp
    type: roundrobin
`

	ExpectInputHasOutput(t, input, output)
}

func TestServiceDefaultValues(t *testing.T) {
	input := `services:
  - name: svc1
    hosts:
      - foo1.com
    upstream:
      id: httpbin
      name: httpbin
      nodes:
        - host: HTTPBIN_PLACEHOLDER
          port: 80
          weight: 1
`
	output := `services:
- hosts:
  - foo1.com
  id: ""
  name: svc1
  upstream:
    hash_on: vars
    id: httpbin
    name: httpbin
    nodes:
    - host: HTTPBIN_PLACEHOLDER
      port: 80
      priority: 0
      weight: 1
    pass_host: pass
    scheme: http
    type: roundrobin
`
	ExpectInputHasOutput(t, input, output)
}
