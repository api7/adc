package db

import (
	"errors"
	"github.com/api7/adc/pkg/data"
	"github.com/hashicorp/go-memdb"
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
				"name": {
					Name:    "name",
					Unique:  true,
					Indexer: &memdb.StringFieldIndex{Field: "Name"},
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

func NewMemDB(configure *data.Configuration) (*DB, error) {
	db, err := memdb.NewMemDB(schema)
	if err != nil {
		return nil, err
	}

	txn := db.Txn(true)

	for _, service := range configure.Services {
		if service.ID == "" {
			service.ID = service.Name
		}

		err := txn.Insert("services", service)
		if err != nil {
			return nil, err
		}
	}
	txn.Commit()

	return &DB{memDB: db}, nil
}

func (db *DB) GetServiceByID(id string) (*data.Service, error) {
	obj, err := db.memDB.Txn(false).First("services", "id", id)
	if err != nil {
		return nil, err
	}

	if obj == nil {
		return nil, NotFound
	}

	return obj.(*data.Service), err
}
