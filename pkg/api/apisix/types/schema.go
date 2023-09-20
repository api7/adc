package types

import (
	_ "embed"
	"encoding/json"
)

var (
	//go:embed data/default_values.json
	DefaultValues []byte

	PluginDefaultValues map[string]Plugin
)

func init() {
	err := json.Unmarshal(DefaultValues, &PluginDefaultValues)
	if err != nil {
		panic("failed to parse plugin defaults values")
	}
}

// Return true if the value is primitive types or array of primitive types,
// and very simple objects (without reserved keys)
func isBasicTypes(value interface{}) bool {
	switch t := value.(type) {
	case bool:
		return true
	case float64:
		return true
	case string:
		return true
	case []interface{}:
		for _, v := range t {
			if !isBasicTypes(v) {
				return false
			}
		}
		return true
		//case map[string]interface{}:
		//	for k, v := range t {
		//		// we can do this trick because this function validates only the json we generated
		//		if k == "type" && v == "global" {
		//			continue
		//		}
		//		if _, ok := ReservedKeys[k]; ok {
		//			return false
		//		}
		//	}
		//	return true
	}
	return false
}

// isArrayWithSchema returns the item schema if the defaultObject contains:
// key "type" == "array", has "items" key and the key is an object
// Otherwise, returns nil
func isArrayWithSchema(defaultObject map[string]interface{}) map[string]interface{} {
	schemaType, hasType := defaultObject["type"]
	items, hasArraySchema := defaultObject["items"]

	if !(hasType && schemaType == "array" && hasArraySchema) {
		return nil
	}

	itemDefaultSchema, ok := items.(map[string]interface{})
	if !ok {
		return nil
	}

	return itemDefaultSchema
}

// ReservedKeys of default values generated from json schema
// Special cases:
// "google-cloud-logging": resource.type = global
// Other keys already checked manually
var (
	ReservedKeys = map[string]struct{}{
		"if":           {},
		"then":         {},
		"else":         {},
		"default":      {},
		"type":         {},
		"properties":   {},
		"dependencies": {},
		"items":        {},
	}
)

func isReservedKey(k string, object map[string]interface{}) bool {
	if k == "type" {
		v, ok := object[k]
		if ok && (v == "array") {
			// we have only "array" remains in the schema
			return true
		}
		return false
	}

	_, ok := ReservedKeys[k]
	return ok
}

func SetArrayDefaultValue(array []interface{}, schema map[string]interface{}) []interface{} {
	if array == nil {
		// it shouldn't be nil, but we still check here
		array = []interface{}{}
	}

	for i, item := range array {
		itemObj, ok := item.(map[string]interface{})
		if ok {
			array[i] = SetDefaultValue(itemObj, schema)
		}
	}

	return array
}

func SetDefaultValue(object, defaultObject map[string]interface{}) map[string]interface{} {
	if object == nil {
		object = make(map[string]interface{})
	}

	// TODO: Should merge default and properties, but we don't have such schema, so leave it here for now
	if properties, hasProperties := defaultObject["properties"]; hasProperties {
		if propertiesObj, ok := properties.(map[string]interface{}); ok {
			return propertiesObj
		}
		return object
	}
	if def, hasDefault := defaultObject["default"]; hasDefault {
		if defObj, ok := def.(map[string]interface{}); ok {
			return defObj
		}
		return object
	}

	// process keys
	for key, defaultValue := range defaultObject {
		if isReservedKey(key, object) {
			continue
		}
		value, ok := object[key]
		if !ok {
			// doesn't exist, simple set the value
			if isBasicTypes(defaultValue) {
				object[key] = defaultValue
			} else {
				defaultObj, ok := defaultValue.(map[string]interface{})
				if ok {
					arraySchema := isArrayWithSchema(defaultObj)
					if arraySchema != nil {
						// do nothing, because empty array with schema doesn't have default value
					} else {
						object[key] = SetDefaultValue(nil, defaultObj)
					}
				}
			}
		} else {
			// ok should always true
			defaultObj, ok := defaultValue.(map[string]interface{})
			if !isBasicTypes(defaultValue) && ok {
				// is obj
				obj, isObj := value.(map[string]interface{})

				// is an array
				itemArray, isArray := value.([]interface{})
				itemDefaultSchema := isArrayWithSchema(defaultObj)

				if isObj {
					// is object, should iterate recursively
					object[key] = SetDefaultValue(obj, defaultObj)
				} else if isArray && len(itemArray) > 0 && itemDefaultSchema != nil {
					object[key] = SetArrayDefaultValue(itemArray, itemDefaultSchema)
				}
			}
		}
	}

	return object
}

func GetPluginDefaultValues(name string, value Plugin) Plugin {
	if value == nil {
		value = make(Plugin)
	}
	defaultObject, ok := PluginDefaultValues[name]
	if !ok {
		return value
	}

	if len(value) == 0 {
		if ok {
			return SetDefaultValue(nil, defaultObject)
		}
		return value
	}

	value = SetDefaultValue(value, defaultObject)
	return SpecialPatches(name, value)
}

func SpecialPatches(name string, value Plugin) Plugin {
	setDefaultValue := func(object Plugin, key string, value interface{}) {
		_, ok := object[key]
		if !ok {
			object[key] = value
		}
	}

	if name == "limit-count" {
		policy, ok := value["policy"]
		if ok {
			if policy == "redis" {
				setDefaultValue(value, "redis_database", 0)
				setDefaultValue(value, "redis_port", 6379)
				setDefaultValue(value, "redis_ssl", false)
				setDefaultValue(value, "redis_ssl_verify", false)
				setDefaultValue(value, "redis_timeout", 1000)
			} else if policy == "redis-cluster" {
				setDefaultValue(value, "redis_cluster_ssl", false)
				setDefaultValue(value, "redis_cluster_ssl_verify", false)
				setDefaultValue(value, "redis_timeout", 1000)
			}
		}
	}

	return value
}
