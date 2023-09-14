package db

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

func TestGetServiceByID(t *testing.T) {
	// Test Case 1: get service by id
	config := types.Configuration{
		Services: []*types.Service{svc},
	}

	db, _ := NewMemDB(&config)
	service, err := db.GetServiceByID("svc")
	assert.Nil(t, err, "check the error")
	assert.Equal(t, svc, service, "check the service")

	// Test Case 2: get service by id (not found)
	_, err = db.GetServiceByID("not-found")
	assert.Equal(t, NotFound, err, "check the error")

	// Test Case 3: Service don't have id
	svc1 := *svc
	svc1.ID = ""
	config = types.Configuration{
		Services: []*types.Service{&svc1},
	}

	db, _ = NewMemDB(&config)
	service, err = db.GetServiceByID("svc")
	assert.Nil(t, err, "check the error")
	assert.Equal(t, svc, service, "check the service")
}

func TestGetRouteByID(t *testing.T) {
	// Test Case 1: get route by id
	config := types.Configuration{
		Routes: []*types.Route{route},
	}

	db, _ := NewMemDB(&config)
	route1, err := db.GetRouteByID("route")
	assert.Nil(t, err, "check the error")
	assert.Equal(t, route, route1, "check the route")

	// Test Case 2: get route by id (not found)
	_, err = db.GetRouteByID("not-found")
	assert.Equal(t, NotFound, err, "check the error")

	// Test Case 3: Route don't have id
	route.ID = ""
	config = types.Configuration{
		Routes: []*types.Route{route},
	}

	db, _ = NewMemDB(&config)
	route1, err = db.GetRouteByID("route")
	assert.Nil(t, err, "check the error")
	assert.Equal(t, route, route1, "check the route")
}
