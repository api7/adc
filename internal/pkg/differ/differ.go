package differ

import (
	"reflect"

	"github.com/api7/adc/internal/pkg/db"
	"github.com/api7/adc/pkg/data"
)

// Differ is the object of comparing two configurations.
type Differ struct {
	localDB      *db.DB
	localConfig  *data.Configuration
	remoteConfig *data.Configuration

	evenChan *data.Event
}

// NewDiffer creates a new Differ object.
func NewDiffer(local, remote *data.Configuration) (*Differ, error) {
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
	events = append(events, serviceEvents...)
	events = append(events, routeEvents...)

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
					Value:        remoteSvc,
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
					Value:        remoteRoute,
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
