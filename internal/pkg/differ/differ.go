package differ

import (
	"fmt"
	"reflect"
	"sort"

	"github.com/api7/adc/internal/pkg/db"
	"github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/pkg/data"
)

var _orderIndex = -1

func _order() int {
	_orderIndex += 1
	return _orderIndex
}

func _key(typ data.ResourceType, option int) string {
	return fmt.Sprintf("%s:%d", typ, option)
}

// order is the events order to ensure the data dependency. Higher takes priority
// route requires: service, plugin config, consumer (soft require)
// consumer requires: consumer group
// The dependent resources should be created/updated first but deleted later
var order = map[string]int{
	_key(data.ServiceResourceType, data.DeleteOption):       _order(),
	_key(data.PluginConfigResourceType, data.DeleteOption):  _order(),
	_key(data.ConsumerGroupResourceType, data.DeleteOption): _order(),
	_key(data.ConsumerResourceType, data.DeleteOption):      _order(),
	_key(data.RouteResourceType, data.DeleteOption):         _order(),

	_key(data.RouteResourceType, data.UpdateOption):         _order(),
	_key(data.ServiceResourceType, data.UpdateOption):       _order(),
	_key(data.PluginConfigResourceType, data.UpdateOption):  _order(),
	_key(data.ConsumerResourceType, data.UpdateOption):      _order(),
	_key(data.ConsumerGroupResourceType, data.UpdateOption): _order(),

	_key(data.RouteResourceType, data.CreateOption):         _order(),
	_key(data.ServiceResourceType, data.CreateOption):       _order(),
	_key(data.PluginConfigResourceType, data.CreateOption):  _order(),
	_key(data.ConsumerResourceType, data.CreateOption):      _order(),
	_key(data.ConsumerGroupResourceType, data.CreateOption): _order(),

	// no dependency
	_key(data.SSLResourceType, data.DeleteOption):            _order(),
	_key(data.SSLResourceType, data.CreateOption):            _order(),
	_key(data.SSLResourceType, data.UpdateOption):            _order(),
	_key(data.GlobalRuleResourceType, data.DeleteOption):     _order(),
	_key(data.GlobalRuleResourceType, data.CreateOption):     _order(),
	_key(data.GlobalRuleResourceType, data.UpdateOption):     _order(),
	_key(data.PluginMetadataResourceType, data.DeleteOption): _order(),
	_key(data.PluginMetadataResourceType, data.CreateOption): _order(),
	_key(data.PluginMetadataResourceType, data.UpdateOption): _order(),
}

// Differ is the object of comparing two configurations.
type Differ struct {
	localDB      *db.DB
	localConfig  *types.Configuration
	remoteConfig *types.Configuration
}

// NewDiffer creates a new Differ object.
func NewDiffer(local, remote *types.Configuration) (*Differ, error) {
	db, err := db.NewMemDB(local)
	if err != nil {
		return nil, err
	}

	return &Differ{
		localDB:      db,
		localConfig:  local,
		remoteConfig: remote,
	}, nil
}

// sortEvents sorts events descending, higher priority events will be executed first
func sortEvents(events []*data.Event) {
	sort.Slice(events, func(i, j int) bool {
		return order[_key(events[i].ResourceType, events[i].Option)] > order[_key(events[j].ResourceType, events[j].Option)]
	})
}

// Diff compares the local configuration and remote configuration, and returns the events.
func (d *Differ) Diff() ([]*data.Event, error) {
	var events []*data.Event
	var err error

	serviceEvents, err := d.diffServices()
	if err != nil {
		return nil, err
	}

	routeEvents, err := d.diffRoutes()
	if err != nil {
		return nil, err
	}

	consumerEvents, err := d.diffConsumers()
	if err != nil {
		return nil, err
	}

	sslEvents, err := d.diffSSLs()
	if err != nil {
		return nil, err
	}

	globalRuleEvents, err := d.diffGlobalRule()
	if err != nil {
		return nil, err
	}

	pluginConfigEvents, err := d.diffPluginConfig()
	if err != nil {
		return nil, err
	}

	consumerGroupEvents, err := d.diffConsumerGroup()
	if err != nil {
		return nil, err
	}

	pluginMetadataEvents, err := d.diffPluginMetadata()
	if err != nil {
		return nil, err
	}

	events = append(events, serviceEvents...)
	events = append(events, routeEvents...)
	events = append(events, consumerEvents...)
	events = append(events, sslEvents...)
	events = append(events, globalRuleEvents...)
	events = append(events, pluginConfigEvents...)
	events = append(events, pluginMetadataEvents...)
	events = append(events, consumerGroupEvents...)

	sortEvents(events)

	return events, nil
}

// diffService compares the services between local and remote.
func (d *Differ) diffServices() ([]*data.Event, error) {
	var events []*data.Event
	var mark = make(map[string]bool)

	for _, remoteSvc := range d.remoteConfig.Services {
		localSvc, err := d.localDB.GetServiceByID(remoteSvc.ID)
		if err != nil {
			// If we can't find the service in local, it means the service should be deleted.
			// So we add a delete event and the value is the service from remote.
			if err == db.NotFound {
				e := data.Event{
					ResourceType: data.ServiceResourceType,
					Option:       data.DeleteOption,
					OldValue:     remoteSvc,
				}
				events = append(events, &e)
				continue
			}

			return nil, err
		}

		mark[localSvc.ID] = true
		// If the service is equal, we don't need to add an event.
		// Else, we use the local service to update the remote service.
		if equal := reflect.DeepEqual(localSvc, remoteSvc); equal {
			continue
		}

		events = append(events, &data.Event{
			ResourceType: data.ServiceResourceType,
			Option:       data.UpdateOption,
			OldValue:     remoteSvc,
			Value:        localSvc,
		})
	}

	// If the service is not in the remote configuration, it means the service should be created.
	for _, service := range d.localConfig.Services {
		if mark[service.ID] {
			continue
		}

		events = append(events, &data.Event{
			ResourceType: data.ServiceResourceType,
			Option:       data.CreateOption,
			Value:        service,
		})
	}

	return events, nil
}

// diffRoutes compares the routes between local and remote.
func (d *Differ) diffRoutes() ([]*data.Event, error) {
	var events []*data.Event
	var mark = make(map[string]bool)

	for _, remoteRoute := range d.remoteConfig.Routes {
		localRoute, err := d.localDB.GetRouteByID(remoteRoute.ID)
		if err != nil {
			// If we can't find the route in local, it means the route should be deleted.
			// So we add a delete event and the value is the route from remote.
			if err == db.NotFound {
				e := data.Event{
					ResourceType: data.RouteResourceType,
					Option:       data.DeleteOption,
					OldValue:     remoteRoute,
				}
				events = append(events, &e)
				continue
			}

			return nil, err
		}

		mark[localRoute.ID] = true
		// If the route is equal, we don't need to add an event.
		// Else, we use the local routes to update the remote routes.
		if equal := reflect.DeepEqual(localRoute, remoteRoute); equal {
			continue
		}

		events = append(events, &data.Event{
			ResourceType: data.RouteResourceType,
			Option:       data.UpdateOption,
			OldValue:     remoteRoute,
			Value:        localRoute,
		})
	}

	// If the route is not in the remote configuration, it means the route should be created.
	for _, route := range d.localConfig.Routes {
		if mark[route.ID] {
			continue
		}

		events = append(events, &data.Event{
			ResourceType: data.RouteResourceType,
			Option:       data.CreateOption,
			Value:        route,
		})
	}

	return events, nil
}

// diffConsumers compares the consumers between local and remote.
func (d *Differ) diffConsumers() ([]*data.Event, error) {
	var events []*data.Event
	var mark = make(map[string]bool)

	for _, remoteConsumers := range d.remoteConfig.Consumers {
		localConsumer, err := d.localDB.GetConsumerByID(remoteConsumers.Username)
		if err != nil {
			// we can't find in local config, should delete it
			if err == db.NotFound {
				e := data.Event{
					ResourceType: data.ConsumerResourceType,
					Option:       data.DeleteOption,
					OldValue:     remoteConsumers,
				}
				events = append(events, &e)
				continue
			}

			return nil, err
		}

		mark[localConsumer.Username] = true
		// skip when equals
		if equal := reflect.DeepEqual(localConsumer, remoteConsumers); equal {
			continue
		}

		// otherwise update
		events = append(events, &data.Event{
			ResourceType: data.ConsumerResourceType,
			Option:       data.UpdateOption,
			OldValue:     remoteConsumers,
			Value:        localConsumer,
		})
	}

	// only in local, create
	for _, consumer := range d.localConfig.Consumers {
		if mark[consumer.Username] {
			continue
		}

		events = append(events, &data.Event{
			ResourceType: data.ConsumerResourceType,
			Option:       data.CreateOption,
			Value:        consumer,
		})
	}

	return events, nil
}

// diffSSLs compares the routes between local and remote.
func (d *Differ) diffSSLs() ([]*data.Event, error) {
	var events []*data.Event
	var mark = make(map[string]bool)

	for _, remoteSSL := range d.remoteConfig.SSLs {
		localSSL, err := d.localDB.GetSSLByID(remoteSSL.ID)
		if err != nil {
			// we can't find the route in local, it means the route should be deleted.
			if err == db.NotFound {
				e := data.Event{
					ResourceType: data.SSLResourceType,
					Option:       data.DeleteOption,
					OldValue:     remoteSSL,
				}
				events = append(events, &e)
				continue
			}

			return nil, err
		}

		mark[localSSL.ID] = true
		// skip when equals
		if equal := reflect.DeepEqual(localSSL, remoteSSL); equal {
			continue
		}

		events = append(events, &data.Event{
			ResourceType: data.SSLResourceType,
			Option:       data.UpdateOption,
			OldValue:     remoteSSL,
			Value:        localSSL,
		})
	}

	// only in local, create
	for _, ssl := range d.localConfig.SSLs {
		if mark[ssl.ID] {
			continue
		}

		events = append(events, &data.Event{
			ResourceType: data.SSLResourceType,
			Option:       data.CreateOption,
			Value:        ssl,
		})
	}

	return events, nil
}

// diffGlobalRule compares the global_rules between local and remote.
func (d *Differ) diffGlobalRule() ([]*data.Event, error) {
	var events []*data.Event
	var mark = make(map[string]bool)

	for _, remoteGlobalRule := range d.remoteConfig.GlobalRules {
		localGlobalRule, err := d.localDB.GetGlobalRuleByID(remoteGlobalRule.ID)
		if err != nil {
			// we can't find in local config, should delete it
			if err == db.NotFound {
				e := data.Event{
					ResourceType: data.GlobalRuleResourceType,
					Option:       data.DeleteOption,
					OldValue:     remoteGlobalRule,
				}
				events = append(events, &e)
				continue
			}

			return nil, err
		}

		mark[localGlobalRule.ID] = true
		// skip when equals
		if equal := reflect.DeepEqual(localGlobalRule, remoteGlobalRule); equal {
			continue
		}

		// otherwise update
		events = append(events, &data.Event{
			ResourceType: data.GlobalRuleResourceType,
			Option:       data.UpdateOption,
			OldValue:     remoteGlobalRule,
			Value:        localGlobalRule,
		})
	}

	// only in local, create
	for _, globalRule := range d.localConfig.GlobalRules {
		if mark[globalRule.ID] {
			continue
		}

		events = append(events, &data.Event{
			ResourceType: data.GlobalRuleResourceType,
			Option:       data.CreateOption,
			Value:        globalRule,
		})
	}

	return events, nil
}

// diffPluginConfig compares the global_rules between local and remote.
func (d *Differ) diffPluginConfig() ([]*data.Event, error) {
	var events []*data.Event
	var mark = make(map[string]bool)

	for _, remotePluginConfig := range d.remoteConfig.PluginConfigs {
		localPluginConfig, err := d.localDB.GetPluginConfigByID(remotePluginConfig.ID)
		if err != nil {
			// we can't find in local config, should delete it
			if err == db.NotFound {
				e := data.Event{
					ResourceType: data.PluginConfigResourceType,
					Option:       data.DeleteOption,
					OldValue:     remotePluginConfig,
				}
				events = append(events, &e)
				continue
			}

			return nil, err
		}

		mark[localPluginConfig.ID] = true
		// skip when equals
		if equal := reflect.DeepEqual(localPluginConfig, remotePluginConfig); equal {
			continue
		}

		// otherwise update
		events = append(events, &data.Event{
			ResourceType: data.PluginConfigResourceType,
			Option:       data.UpdateOption,
			OldValue:     remotePluginConfig,
			Value:        localPluginConfig,
		})
	}

	// only in local, create
	for _, pluginConfig := range d.localConfig.PluginConfigs {
		if mark[pluginConfig.ID] {
			continue
		}

		events = append(events, &data.Event{
			ResourceType: data.PluginConfigResourceType,
			Option:       data.CreateOption,
			Value:        pluginConfig,
		})
	}

	return events, nil
}

// diffConsumerGroup compares the global_rules between local and remote.
func (d *Differ) diffConsumerGroup() ([]*data.Event, error) {
	var events []*data.Event
	var mark = make(map[string]bool)

	for _, remoteConsumerGroup := range d.remoteConfig.ConsumerGroups {
		localConsumerGroup, err := d.localDB.GetConsumerGroupByID(remoteConsumerGroup.ID)
		if err != nil {
			// we can't find in local config, should delete it
			if err == db.NotFound {
				e := data.Event{
					ResourceType: data.ConsumerGroupResourceType,
					Option:       data.DeleteOption,
					OldValue:     remoteConsumerGroup,
				}
				events = append(events, &e)
				continue
			}

			return nil, err
		}

		mark[localConsumerGroup.ID] = true
		// skip when equals
		if equal := reflect.DeepEqual(localConsumerGroup, remoteConsumerGroup); equal {
			continue
		}

		// otherwise update
		events = append(events, &data.Event{
			ResourceType: data.ConsumerGroupResourceType,
			Option:       data.UpdateOption,
			OldValue:     remoteConsumerGroup,
			Value:        localConsumerGroup,
		})
	}

	// only in local, create
	for _, consumerGroup := range d.localConfig.ConsumerGroups {
		if mark[consumerGroup.ID] {
			continue
		}

		events = append(events, &data.Event{
			ResourceType: data.ConsumerGroupResourceType,
			Option:       data.CreateOption,
			Value:        consumerGroup,
		})
	}

	return events, nil
}

// diffPluginMetadata compares the global_rules between local and remote.
func (d *Differ) diffPluginMetadata() ([]*data.Event, error) {
	var events []*data.Event
	var mark = make(map[string]bool)

	for _, remotePluginMetadata := range d.remoteConfig.PluginMetadatas {
		localPluginMetadata, err := d.localDB.GetPluginMetadataByID(remotePluginMetadata.ID)
		if err != nil {
			// we can't find in local config, should delete it
			if err == db.NotFound {
				e := data.Event{
					ResourceType: data.PluginMetadataResourceType,
					Option:       data.DeleteOption,
					OldValue:     remotePluginMetadata,
				}
				events = append(events, &e)
				continue
			}

			return nil, err
		}

		mark[localPluginMetadata.ID] = true
		// skip when equals
		if equal := reflect.DeepEqual(localPluginMetadata, remotePluginMetadata); equal {
			continue
		}

		// otherwise update
		events = append(events, &data.Event{
			ResourceType: data.PluginMetadataResourceType,
			Option:       data.UpdateOption,
			OldValue:     remotePluginMetadata,
			Value:        localPluginMetadata,
		})
	}

	// only in local, create
	for _, pluginMetadata := range d.localConfig.PluginMetadatas {
		if mark[pluginMetadata.ID] {
			continue
		}

		events = append(events, &data.Event{
			ResourceType: data.PluginMetadataResourceType,
			Option:       data.CreateOption,
			Value:        pluginMetadata,
		})
	}

	return events, nil
}
