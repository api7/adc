package differ

import (
	"net/http"
	"testing"

	"github.com/stretchr/testify/assert"

	"github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/pkg/data"
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

func TestSortEvents(t *testing.T) {
	events := []*data.Event{
		{
			ResourceType: data.RouteResourceType,
			Option:       data.CreateOption,
		},
		{
			ResourceType: data.ServiceResourceType,
			Option:       data.CreateOption,
		},
		{
			ResourceType: data.RouteResourceType,
			Option:       data.DeleteOption,
		},
		{
			ResourceType: data.ServiceResourceType,
			Option:       data.DeleteOption,
		},
		{
			ResourceType: data.RouteResourceType,
			Option:       data.UpdateOption,
		},
		{
			ResourceType: data.ServiceResourceType,
			Option:       data.UpdateOption,
		},
	}

	sortEvents(events)
	assert.Equal(t, []*data.Event{
		{
			ResourceType: data.ServiceResourceType,
			Option:       data.CreateOption,
		},
		{
			ResourceType: data.RouteResourceType,
			Option:       data.CreateOption,
		},
		{
			ResourceType: data.ServiceResourceType,
			Option:       data.UpdateOption,
		},
		{
			ResourceType: data.RouteResourceType,
			Option:       data.UpdateOption,
		},
		{
			ResourceType: data.RouteResourceType,
			Option:       data.DeleteOption,
		},
		{
			ResourceType: data.ServiceResourceType,
			Option:       data.DeleteOption,
		},
	}, events, "check the content of sorted events")

}

func TestDiff(t *testing.T) {
	// Test case 1: delete events
	localConfig := &types.Configuration{
		Services: []*types.Service{},
		Routes:   []*types.Route{route},
	}

	route1 := *route
	route1.ID = "route1"
	route1.Name = "route1"
	remoteConfig := &types.Configuration{
		Services: []*types.Service{svc},
		Routes:   []*types.Route{&route1},
	}

	differ, _ := NewDiffer(localConfig, remoteConfig)
	events, _ := differ.Diff()
	assert.Equal(t, 3, len(events), "check the number of delete events")
	assert.Equal(t, []*data.Event{
		{
			ResourceType: data.RouteResourceType,
			Option:       data.CreateOption,
			Value:        route,
		},
		{
			ResourceType: data.RouteResourceType,
			Option:       data.DeleteOption,
			OldValue:     &route1,
		},
		{
			ResourceType: data.ServiceResourceType,
			Option:       data.DeleteOption,
			OldValue:     svc,
		},
	}, events, "check the content of delete events")
}

func TestDiffServices(t *testing.T) {
	// Test case 1: delete events
	localConfig := &types.Configuration{
		Services: []*types.Service{},
	}
	remoteConfig := &types.Configuration{
		Services: []*types.Service{svc},
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
	localConfig = &types.Configuration{
		Services: []*types.Service{svc},
	}
	svc1 := *svc
	svc1.Name = "svc1"
	svc1.Hosts[0] = "svc1.example.com"
	remoteConfig = &types.Configuration{
		Services: []*types.Service{&svc1},
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
	localConfig = &types.Configuration{
		Services: []*types.Service{svc},
	}
	remoteConfig = &types.Configuration{
		Services: []*types.Service{},
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
	localConfig = &types.Configuration{
		Services: []*types.Service{svc},
	}

	remoteConfig = &types.Configuration{
		Services: []*types.Service{svc},
	}

	differ, _ = NewDiffer(localConfig, remoteConfig)
	events, _ = differ.diffServices()
	assert.Equal(t, 0, len(events), "check the number of no events")

	// Test case 5: delete and create events
	localConfig = &types.Configuration{
		Services: []*types.Service{svc},
	}
	svc1 = *svc
	svc1.ID = "svc1"
	svc1.Name = "svc1"
	svc1.Hosts[0] = "svc1.example.com"
	remoteConfig = &types.Configuration{
		Services: []*types.Service{&svc1},
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
	localConfig := &types.Configuration{
		Routes: []*types.Route{},
	}
	remoteConfig := &types.Configuration{
		Routes: []*types.Route{route},
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
	localConfig = &types.Configuration{
		Routes: []*types.Route{route},
	}
	route1 := *route
	route1.Name = "route1"
	remoteConfig = &types.Configuration{
		Routes: []*types.Route{&route1},
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
	localConfig = &types.Configuration{
		Routes: []*types.Route{route},
	}
	remoteConfig = &types.Configuration{
		Routes: []*types.Route{},
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
	localConfig = &types.Configuration{
		Routes: []*types.Route{},
	}

	remoteConfig = &types.Configuration{
		Routes: []*types.Route{},
	}

	differ, _ = NewDiffer(localConfig, remoteConfig)
	events, _ = differ.diffRoutes()
	assert.Equal(t, 0, len(events), "check the number of no events")

	// Test case 5: delete and create events
	localConfig = &types.Configuration{
		Routes: []*types.Route{route},
	}
	route1 = *route
	route1.ID = "route1"
	route1.Name = "route1"
	remoteConfig = &types.Configuration{
		Routes: []*types.Route{&route1},
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
