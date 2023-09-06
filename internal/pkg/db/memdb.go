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

	for _, globalRule := range config.GlobalRules {
		err = txn.Insert("global_rules", globalRule)
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

func (db *DB) GetGlobalRuleByID(username string) (*types.GlobalRule, error) {
	return getByID[types.GlobalRule](db, "global_rules", username)
}
