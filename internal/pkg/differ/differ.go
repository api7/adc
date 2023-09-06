package differ

import (
	"fmt"
	"reflect"
	"sort"

	"github.com/api7/adc/internal/pkg/db"
	"github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/pkg/data"
)

func _key(typ data.ResourceType, option int) string {
	return fmt.Sprintf("%s:%d", typ, option)
}

// Since the routes is related to the services, we need to sort the events.
// The order is:
// 0. Consumers Delete
// 1. Services Create
// 2. Routes Create
// 3. Services Update
// 4. Routes Update
// 5. Routes Delete
// 6. Services Delete
// 7. Consumers Create
// 8. Consumers Update
// 9. SSLs Delete
// 10. SSLs Create
// 11. SSLs Update
// 12. GlobalRules Delete
// 13. GlobalRules Create
// 11. GlobalRules Update
var order = map[string]int{
	_key(data.GlobalRuleResourceType, data.UpdateOption): 14,
	_key(data.GlobalRuleResourceType, data.CreateOption): 13,
	_key(data.GlobalRuleResourceType, data.DeleteOption): 12,
	_key(data.SSLResourceType, data.UpdateOption):        11,
	_key(data.SSLResourceType, data.CreateOption):        10,
	_key(data.SSLResourceType, data.DeleteOption):        9,
	_key(data.ConsumerResourceType, data.UpdateOption):   8,
	_key(data.ConsumerResourceType, data.CreateOption):   7,
	_key(data.ServiceResourceType, data.CreateOption):    6,
	_key(data.RouteResourceType, data.CreateOption):      5,
	_key(data.ServiceResourceType, data.UpdateOption):    4,
	_key(data.RouteResourceType, data.UpdateOption):      3,
	_key(data.RouteResourceType, data.DeleteOption):      2,
	_key(data.ServiceResourceType, data.DeleteOption):    1,
	_key(data.ConsumerResourceType, data.DeleteOption):   0,
}

// Differ is the object of comparing two configurations.
type Differ struct {
	localDB      *db.DB
	localConfig  *types.Configuration
	remoteConfig *types.Configuration

	evenChan *data.Event
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

	events = append(events, serviceEvents...)
	events = append(events, routeEvents...)
	events = append(events, consumerEvents...)
	events = append(events, sslEvents...)
	events = append(events, globalRuleEvents...)

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
