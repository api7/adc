package db

import (
	"github.com/api7/adc/pkg/data"
	"github.com/stretchr/testify/assert"
	"testing"
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
	svc.ID = ""
	svc.Name = "test"
	config = data.Configuration{
		Services: []*data.Service{svc},
	}

	db, _ = NewMemDB(&config)
	service, err = db.GetServiceByID("test")
	assert.Nil(t, err, "check the error")
	assert.Equal(t, svc, service, "check the service")
}
