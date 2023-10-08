package db

import (
	"errors"

	"github.com/hashicorp/go-memdb"

	"github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/pkg/common"
)

var schema = &memdb.DBSchema{
	Tables: map[string]*memdb.TableSchema{
		"services": {
			Name: "services",
			Indexes: map[string]*memdb.IndexSchema{
				"id": {
					Name:    "id",
					Unique:  true,
					Indexer: &memdb.StringFieldIndex{Field: "ID"},
				},
			},
		},
		"routes": {
			Name: "routes",
			Indexes: map[string]*memdb.IndexSchema{
				"id": {
					Name:    "id",
					Unique:  true,
					Indexer: &memdb.StringFieldIndex{Field: "ID"},
				},
			},
		},
		"consumers": {
			Name: "consumers",
			Indexes: map[string]*memdb.IndexSchema{
				"id": {
					Name:    "id",
					Unique:  true,
					Indexer: &memdb.StringFieldIndex{Field: "Username"},
				},
			},
		},
		"ssls": {
			Name: "ssls",
			Indexes: map[string]*memdb.IndexSchema{
				"id": {
					Name:    "id",
					Unique:  true,
					Indexer: &memdb.StringFieldIndex{Field: "ID"},
				},
			},
		},
		"global_rules": {
			Name: "global_rules",
			Indexes: map[string]*memdb.IndexSchema{
				"id": {
					Name:    "id",
					Unique:  true,
					Indexer: &memdb.StringFieldIndex{Field: "ID"},
				},
			},
		},
		"plugin_configs": {
			Name: "plugin_configs",
			Indexes: map[string]*memdb.IndexSchema{
				"id": {
					Name:    "id",
					Unique:  true,
					Indexer: &memdb.StringFieldIndex{Field: "ID"},
				},
			},
		},
		"consumer_groups": {
			Name: "consumer_groups",
			Indexes: map[string]*memdb.IndexSchema{
				"id": {
					Name:    "id",
					Unique:  true,
					Indexer: &memdb.StringFieldIndex{Field: "ID"},
				},
			},
		},
		"plugin_metadatas": {
			Name: "plugin_metadatas",
			Indexes: map[string]*memdb.IndexSchema{
				"id": {
					Name:    "id",
					Unique:  true,
					Indexer: &memdb.StringFieldIndex{Field: "ID"},
				},
			},
		},
		"stream_routes": {
			Name: "stream_routes",
			Indexes: map[string]*memdb.IndexSchema{
				"id": {
					Name:    "id",
					Unique:  true,
					Indexer: &memdb.StringFieldIndex{Field: "ID"},
				},
			},
		},
		"upstreams": {
			Name: "upstreams",
			Indexes: map[string]*memdb.IndexSchema{
				"id": {
					Name:    "id",
					Unique:  true,
					Indexer: &memdb.StringFieldIndex{Field: "ID"},
				},
			},
		},
	},
}

type DB struct {
	memDB *memdb.MemDB
}

var (
	NotFound = errors.New("data not found")
)

func NewMemDB(config *types.Configuration) (*DB, error) {
	db, err := memdb.NewMemDB(schema)
	if err != nil {
		return nil, err
	}

	txn := db.Txn(true)

	common.NormalizeConfiguration(config)

	for _, service := range config.Services {
		err = txn.Insert("services", service)
		if err != nil {
			return nil, err
		}
	}

	for _, routes := range config.Routes {
		err = txn.Insert("routes", routes)
		if err != nil {
			return nil, err
		}
	}

	for _, consumers := range config.Consumers {
		err = txn.Insert("consumers", consumers)
		if err != nil {
			return nil, err
		}
	}

	for _, ssls := range config.SSLs {
		err = txn.Insert("ssls", ssls)
		if err != nil {
			return nil, err
		}
	}

	for _, globalRule := range config.GlobalRules {
		err = txn.Insert("global_rules", globalRule)
		if err != nil {
			return nil, err
		}
	}

	for _, pluginConfig := range config.PluginConfigs {
		err = txn.Insert("plugin_configs", pluginConfig)
		if err != nil {
			return nil, err
		}
	}

	for _, consumerGroup := range config.ConsumerGroups {
		err = txn.Insert("consumer_groups", consumerGroup)
		if err != nil {
			return nil, err
		}
	}

	for _, pluginMetadata := range config.PluginMetadatas {
		err = txn.Insert("plugin_metadatas", pluginMetadata)
		if err != nil {
			return nil, err
		}
	}

	for _, streamRoute := range config.StreamRoutes {
		err = txn.Insert("stream_routes", streamRoute)
		if err != nil {
			return nil, err
		}
	}

	for _, upstream := range config.Upstreams {
		err = txn.Insert("upstreams", upstream)
		if err != nil {
			return nil, err
		}
	}

	txn.Commit()

	return &DB{memDB: db}, nil
}

func getByID[T any](db *DB, table, id string) (*T, error) {
	obj, err := db.memDB.Txn(false).First(table, "id", id)
	if err != nil {
		return nil, err
	}

	if obj == nil {
		return nil, NotFound
	}

	return obj.(*T), err
}

func (db *DB) GetServiceByID(id string) (*types.Service, error) {
	return getByID[types.Service](db, "services", id)
}

func (db *DB) GetRouteByID(id string) (*types.Route, error) {
	return getByID[types.Route](db, "routes", id)
}

func (db *DB) GetConsumerByID(username string) (*types.Consumer, error) {
	return getByID[types.Consumer](db, "consumers", username)
}

func (db *DB) GetSSLByID(id string) (*types.SSL, error) {
	return getByID[types.SSL](db, "ssls", id)
}

func (db *DB) GetGlobalRuleByID(id string) (*types.GlobalRule, error) {
	return getByID[types.GlobalRule](db, "global_rules", id)
}

func (db *DB) GetPluginConfigByID(id string) (*types.PluginConfig, error) {
	return getByID[types.PluginConfig](db, "plugin_configs", id)
}

func (db *DB) GetConsumerGroupByID(id string) (*types.ConsumerGroup, error) {
	return getByID[types.ConsumerGroup](db, "consumer_groups", id)
}

func (db *DB) GetPluginMetadataByID(id string) (*types.PluginMetadata, error) {
	return getByID[types.PluginMetadata](db, "plugin_metadatas", id)
}

func (db *DB) GetStreamRouteByID(id string) (*types.StreamRoute, error) {
	return getByID[types.StreamRoute](db, "stream_routes", id)
}

func (db *DB) GetUpstreamByID(id string) (*types.Upstream, error) {
	return getByID[types.Upstream](db, "upstreams", id)
}
