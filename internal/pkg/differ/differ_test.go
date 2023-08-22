package differ

import (
	"net/http"
	"testing"

	"github.com/api7/adc/pkg/data"
	"github.com/stretchr/testify/assert"
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

func TestDiff(t *testing.T) {
	// Test case 1: delete events
	localConfig := &data.Configuration{
		Services: []*data.Service{},
		Routes:   []*data.Route{route},
	}

	route1 := *route
	route1.ID = "route1"
	route1.Name = "route1"
	remoteConfig := &data.Configuration{
		Services: []*data.Service{svc},
		Routes:   []*data.Route{&route1},
	}

	differ, _ := NewDiffer(localConfig, remoteConfig)
	events, _ := differ.Diff()
	assert.Equal(t, 3, len(events), "check the number of delete events")
	assert.Equal(t, []*data.Event{
		{
			ResourceType: data.ServiceResourceType,
			Option:       data.DeleteOption,
			OldValue:     svc,
		},
		{
			ResourceType: data.RouteResourceType,
			Option:       data.DeleteOption,
			OldValue:     &route1,
		},
		{
			ResourceType: data.RouteResourceType,
			Option:       data.CreateOption,
			Value:        route,
		},
	}, events, "check the content of delete events")
}

func TestDiffServices(t *testing.T) {
	// Test case 1: delete events
	localConfig := &data.Configuration{
		Services: []*data.Service{},
	}
	remoteConfig := &data.Configuration{
		Services: []*data.Service{svc},
	}

	differ, _ := NewDiffer(localConfig, remoteConfig)
	events, _ := differ.diffServices()
	assert.Equal(t, 1, len(events), "check the number of delete events")
	assert.Equal(t, []*data.Event{
		{
			ResourceType: data.ServiceResourceType,
			Option:       data.DeleteOption,
			OldValue:     svc,
		},
	}, events, "check the content of delete events")

	// Test case 2: update events
	localConfig = &data.Configuration{
		Services: []*data.Service{svc},
	}
	svc1 := *svc
	svc1.Name = "svc1"
	svc1.Hosts[0] = "svc1.example.com"
	remoteConfig = &data.Configuration{
		Services: []*data.Service{&svc1},
	}

	differ, _ = NewDiffer(localConfig, remoteConfig)
	events, _ = differ.diffServices()
	assert.Equal(t, 1, len(events), "check the number of update events")
	assert.Equal(t, []*data.Event{
		{
			ResourceType: data.ServiceResourceType,
			Option:       data.UpdateOption,
			OldValue:     &svc1,
			Value:        svc,
		},
	}, events, "check the content of update events")

	// Test case 3: create events
	localConfig = &data.Configuration{
		Services: []*data.Service{svc},
	}
	remoteConfig = &data.Configuration{
		Services: []*data.Service{},
	}

	differ, _ = NewDiffer(localConfig, remoteConfig)
	events, _ = differ.diffServices()
	assert.Equal(t, 1, len(events), "check the number of create events")
	assert.Equal(t, []*data.Event{
		{
			ResourceType: data.ServiceResourceType,
			Option:       data.CreateOption,
			Value:        svc,
		},
	}, events, "check the content of create events")

	// Test case 4: no events
	localConfig = &data.Configuration{
		Services: []*data.Service{svc},
	}

	remoteConfig = &data.Configuration{
		Services: []*data.Service{svc},
	}

	differ, _ = NewDiffer(localConfig, remoteConfig)
	events, _ = differ.diffServices()
	assert.Equal(t, 0, len(events), "check the number of no events")

	// Test case 5: delete and create events
	localConfig = &data.Configuration{
		Services: []*data.Service{svc},
	}
	svc1 = *svc
	svc1.ID = "svc1"
	svc1.Name = "svc1"
	svc1.Hosts[0] = "svc1.example.com"
	remoteConfig = &data.Configuration{
		Services: []*data.Service{&svc1},
	}

	differ, _ = NewDiffer(localConfig, remoteConfig)
	events, _ = differ.diffServices()
	assert.Equal(t, 2, len(events), "check the number of delete and create events")
	assert.Equal(t, []*data.Event{
		{
			ResourceType: data.ServiceResourceType,
			Option:       data.DeleteOption,
			OldValue:     &svc1,
		},
		{
			ResourceType: data.ServiceResourceType,
			Option:       data.CreateOption,
			Value:        svc,
		},
	}, events, "check the content of delete and create events")
}

func TestDifferRoutes(t *testing.T) {
	// Test case 1: delete events
	localConfig := &data.Configuration{
		Routes: []*data.Route{},
	}
	remoteConfig := &data.Configuration{
		Routes: []*data.Route{route},
	}

	differ, _ := NewDiffer(localConfig, remoteConfig)
	events, _ := differ.diffRoutes()
	assert.Equal(t, 1, len(events), "check the number of delete events")
	assert.Equal(t, []*data.Event{
		{
			ResourceType: data.RouteResourceType,
			Option:       data.DeleteOption,
			OldValue:     route,
		},
	}, events, "check the content of delete events")

	// Test case 2: update events
	localConfig = &data.Configuration{
		Routes: []*data.Route{route},
	}
	route1 := *route
	route1.Name = "route1"
	remoteConfig = &data.Configuration{
		Routes: []*data.Route{&route1},
	}

	differ, _ = NewDiffer(localConfig, remoteConfig)
	events, _ = differ.diffRoutes()
	assert.Equal(t, 1, len(events), "check the number of update events")
	assert.Equal(t, []*data.Event{
		{
			ResourceType: data.RouteResourceType,
			Option:       data.UpdateOption,
			OldValue:     &route1,
			Value:        route,
		},
	}, events, "check the content of update events")

	// Test case 3: create events
	localConfig = &data.Configuration{
		Routes: []*data.Route{route},
	}
	remoteConfig = &data.Configuration{
		Routes: []*data.Route{},
	}

	differ, _ = NewDiffer(localConfig, remoteConfig)
	events, _ = differ.diffRoutes()
	assert.Equal(t, 1, len(events), "check the number of create events")
	assert.Equal(t, []*data.Event{
		{
			ResourceType: data.RouteResourceType,
			Option:       data.CreateOption,
			Value:        route,
		},
	}, events, "check the content of create events")

	// Test case 4: no events
	localConfig = &data.Configuration{
		Routes: []*data.Route{},
	}

	remoteConfig = &data.Configuration{
		Routes: []*data.Route{},
	}

	differ, _ = NewDiffer(localConfig, remoteConfig)
	events, _ = differ.diffRoutes()
	assert.Equal(t, 0, len(events), "check the number of no events")

	// Test case 5: delete and create events
	localConfig = &data.Configuration{
		Routes: []*data.Route{route},
	}
	route1 = *route
	route1.ID = "route1"
	route1.Name = "route1"
	remoteConfig = &data.Configuration{
		Routes: []*data.Route{&route1},
	}

	differ, _ = NewDiffer(localConfig, remoteConfig)
	events, _ = differ.diffRoutes()
	assert.Equal(t, 2, len(events), "check the number of delete and create events")
	assert.Equal(t, []*data.Event{
		{
			ResourceType: data.RouteResourceType,
			Option:       data.DeleteOption,
			OldValue:     &route1,
		},
		{
			ResourceType: data.RouteResourceType,
			Option:       data.CreateOption,
			Value:        route,
		},
	}, events, "check the content of delete and create events")
}
