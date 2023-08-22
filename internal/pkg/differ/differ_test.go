package differ

import (
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
)

func TestDiff(t *testing.T) {
	// Test case 1: delete events
	localConfig := &data.Configuration{
		Services: []*data.Service{},
	}
	remoteConfig := &data.Configuration{
		Services: []*data.Service{svc},
	}

	differ, _ := NewDiffer(localConfig, remoteConfig)
	events, _ := differ.Diff()
	assert.Equal(t, 1, len(events), "check the number of delete events")
	assert.Equal(t, []*data.Event{
		{
			ResourceType: data.ServiceResourceType,
			Option:       data.DeleteOption,
			Value:        svc,
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
	events, _ = differ.Diff()
	assert.Equal(t, 1, len(events), "check the number of update events")
	assert.Equal(t, []*data.Event{
		{
			ResourceType: data.ServiceResourceType,
			Option:       data.UpdateOption,
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
	events, _ = differ.Diff()
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
	events, _ = differ.Diff()
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
	events, _ = differ.Diff()
	assert.Equal(t, 2, len(events), "check the number of delete and create events")
	assert.Equal(t, []*data.Event{
		{
			ResourceType: data.ServiceResourceType,
			Option:       data.DeleteOption,
			Value:        &svc1,
		},
		{
			ResourceType: data.ServiceResourceType,
			Option:       data.CreateOption,
			Value:        svc,
		},
	}, events, "check the content of delete and create events")
}
