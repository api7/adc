package data

import (
	"net/http"
	"testing"

	"github.com/stretchr/testify/assert"
)

var (
	svc = &Service{
		ID:   "svc",
		Name: "svc",
		Hosts: []string{
			"svc.example.com",
		},
		Labels: []string{"label1", "label2"},
		Upstreams: []Upstream{
			{
				Name: "upstream1",
				Targets: []UpstreamTarget{
					{
						Host: "httpbin.org",
					},
				},
			},
			{
				Name: "upstream2",
				Targets: []UpstreamTarget{
					{
						Host: "httpbin.org",
					},
				},
			},
		},
		UpstreamInUse: "upstream1",
	}

	route = &Route{
		ID:        "route",
		Name:      "route",
		Labels:    []string{"lable1", "label2"},
		Methods:   []string{http.MethodGet},
		Paths:     []string{"/get"},
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
	output, err := event.Output()
	assert.Nil(t, err, "should not return error")
	assert.Equal(t, "deleting service: \"svc\"", output)

	// Test case 2: create events
	event = &Event{
		ResourceType: ServiceResourceType,
		Option:       CreateOption,
		Value:        svc,
	}
	output, err = event.Output()
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
	output, err = event.Output()
	assert.Nil(t, err, "should not return error")
	assert.Contains(t, output, "updating route: \"route\"", "should contain the route name")
	assert.Contains(t, output, "-\t\"description\": \"\"", "should contain the changes")
	assert.Contains(t, output, "+\t\"description\": \"route1\"", "should contain the changes")
}
