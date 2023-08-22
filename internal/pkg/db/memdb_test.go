package db

import (
	"net/http"
	"testing"

	"github.com/stretchr/testify/assert"

	"github.com/api7/adc/pkg/data"
)

var (
	svc = &data.Service{
		ID:   "svc",
		Name: "svc",
		Hosts: []string{
			"svc.example.com",
		},
		Labels: []string{"label1", "label2"},
		Upstreams: []data.Upstream{
			{
				Name: "upstream1",
				Targets: []data.UpstreamTarget{
					{
						Host: "httpbin.org",
					},
				},
			},
			{
				Name: "upstream2",
				Targets: []data.UpstreamTarget{
					{
						Host: "httpbin.org",
					},
				},
			},
		},
		UpstreamInUse: "upstream1",
	}

	route = &data.Route{
		ID:        "route",
		Name:      "route",
		Labels:    []string{"lable1", "label2"},
		Methods:   []string{http.MethodGet},
		Paths:     []string{"/get"},
		ServiceID: "svc",
	}
)

func TestGetServiceByID(t *testing.T) {
	// Test Case 1: get service by id
	config := data.Configuration{
		Services: []*data.Service{svc},
	}

	db, _ := NewMemDB(&config)
	service, err := db.GetServiceByID("svc")
	assert.Nil(t, err, "check the error")
	assert.Equal(t, svc, service, "check the service")

	// Test Case 2: get service by id (not found)
	service, err = db.GetServiceByID("not-found")
	assert.Equal(t, NotFound, err, "check the error")

	// Test Case 3: Service don't have id
	svc1 := *svc
	svc1.ID = ""
	config = data.Configuration{
		Services: []*data.Service{&svc1},
	}

	db, _ = NewMemDB(&config)
	service, err = db.GetServiceByID("svc")
	assert.Nil(t, err, "check the error")
	assert.Equal(t, svc, service, "check the service")
}

func TestGetRouteByID(t *testing.T) {
	// Test Case 1: get route by id
	config := data.Configuration{
		Routes: []*data.Route{route},
	}

	db, _ := NewMemDB(&config)
	route1, err := db.GetRouteByID("route")
	assert.Nil(t, err, "check the error")
	assert.Equal(t, route, route1, "check the route")

	// Test Case 2: get route by id (not found)
	route1, err = db.GetRouteByID("not-found")
	assert.Equal(t, NotFound, err, "check the error")

	// Test Case 3: Route don't have id
	route.ID = ""
	config = data.Configuration{
		Routes: []*data.Route{route},
	}

	db, _ = NewMemDB(&config)
	route1, err = db.GetRouteByID("route")
	assert.Nil(t, err, "check the error")
	assert.Equal(t, route, route1, "check the route")
}
