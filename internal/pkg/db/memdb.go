package db

import (
	"errors"

	"github.com/hashicorp/go-memdb"

	"github.com/api7/adc/pkg/api/apisix/types"
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
	},
}

type DB struct {
	memDB *memdb.MemDB
}

var (
	NotFound = errors.New("data not found")
)

func NewMemDB(configure *types.Configuration) (*DB, error) {
	db, err := memdb.NewMemDB(schema)
	if err != nil {
		return nil, err
	}

	txn := db.Txn(true)

	for _, service := range configure.Services {
		if service.ID == "" {
			service.ID = service.Name
		}
		err = txn.Insert("services", service)
		if err != nil {
			return nil, err
		}
	}

	for _, routes := range configure.Routes {
		if routes.ID == "" {
			routes.ID = routes.Name
		}
		err = txn.Insert("routes", routes)
		if err != nil {
			return nil, err
		}
	}

	for _, consumers := range configure.Consumers {
		err = txn.Insert("consumers", consumers)
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
