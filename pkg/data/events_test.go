package data

import (
	"net/http"
	"testing"

	"github.com/stretchr/testify/assert"

	"github.com/api7/adc/pkg/api/apisix/types"
)

var (
	svc = &types.Service{
		ID:   "svc",
		Name: "svc",
		Hosts: []string{
			"svc.example.com",
		},
		Labels: map[string]string{
			"label1": "v1",
			"label2": "v2",
		},
		Upstream: types.Upstream{
			Name: "upstream1",
			Nodes: []types.UpstreamNode{
				{
					Host: "httpbin.org",
				},
			},
		},
	}

	route = &types.Route{
		ID:   "route",
		Name: "route",
		Labels: map[string]string{
			"label1": "v1",
			"label2": "v2",
		},
		Methods:   []string{http.MethodGet},
		Uris:      []string{"/get"},
		ServiceID: "svc",
	}
)

func TestEventOutput(t *testing.T) {
	// Test case 1: delete events
	event := &Event{
		ResourceType: ServiceResourceType,
		Option:       DeleteOption,
		OldValue:     svc,
	}
	output, err := event.Output(false)
	assert.Nil(t, err, "should not return error")
	assert.Equal(t, "deleting service: \"svc\"", output)

	// Test case 2: create events
	event = &Event{
		ResourceType: ServiceResourceType,
		Option:       CreateOption,
		Value:        svc,
	}
	output, err = event.Output(false)
	assert.Nil(t, err, "should not return error")
	assert.Equal(t, "creating service: \"svc\"", output)

	// Test case 3: update events
	route1 := *route
	route1.Description = "route1"
	event = &Event{
		ResourceType: RouteResourceType,
		Option:       UpdateOption,
		OldValue:     route,
		Value:        route1,
	}
	output, err = event.Output(false)
	assert.Nil(t, err, "should not return error")
	assert.Contains(t, output, "updating route: \"route\"", "should contain the route name")
	assert.Contains(t, output, "+\t\"desc\": \"route1\"", "should contain the changes")
}
