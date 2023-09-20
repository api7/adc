package types

import (
	"encoding/json"
	"github.com/stretchr/testify/assert"
	"strings"
	"testing"
)

// Special Schemas to test:
// [Done]:
// api-breaker, array of primitive type
// aws-lambda, simple nested object
// request-id, default and properties
// kafka-logger, array type
// google-cloud-logging, special "type" field
// limit-count, if-then-else
// [TODO]:
// traffic-split, array type, anyOf, nested schema
// [leave]:
// response-rewrite, dependencies // We don't care about the dependency, leave here
// grpc-transcode, loggly.severity_map, openid-connect.session, proxy-rewrite.headers(oneOf): additionalProperties=true
//    we don't care the additional properties, leave here

func OutputEqual(t *testing.T, plugin Plugin, expected string) {
	out, err := json.Marshal(plugin)
	assert.Nil(t, err)

	// This also removes spaces inside string
	removeAllSpaces := func(s string) string {
		arr := strings.Split(s, "\n")
		for i, s := range arr {
			s = strings.TrimSpace(s)
			s = strings.ReplaceAll(s, " ", "")

			arr[i] = s
		}
		s = strings.Join(arr, "")

		return s
	}

	assert.Equal(t, removeAllSpaces(expected), removeAllSpaces(string(out)))
}

func TestNoNil(t *testing.T) {
	plugin := GetPluginDefaultValues("doesnt-exist", nil)
	assert.NotNil(t, plugin)
}

func TestNoDefaults(t *testing.T) {
	plugin := Plugin{
		"authorization": map[string]interface{}{
			"iam": map[string]interface{}{
				"service": "service_name",
			},
			"some_field": "some_value",
		},
		"keepalive":      false,
		"keepalive_pool": 100,
	}
	plugin = GetPluginDefaultValues("doesnt-exist", plugin)

	OutputEqual(t, plugin, `{
  "authorization": {
    "iam": {
      "service": "service_name"
    },
	"some_field": "some_value"
  },
  "keepalive": false,
  "keepalive_pool": 100
}`)
}

// Primitive type array
func TestArrayOfPrimitiveTypes(t *testing.T) {
	plugin := GetPluginDefaultValues("api-breaker", nil)

	OutputEqual(t, plugin, `{
  "healthy": {
    "http_statuses": [
      200
    ],
    "successes": 3
  },
  "max_breaker_sec": 300,
  "unhealthy": {
    "failures": 3,
    "http_statuses": [
      500
    ]
  }
}`)

	// == value overwrite ==
	plugin = Plugin{
		"unhealthy": map[string]interface{}{
			"http_statuses": []interface{}{
				400, 500, 600, 700,
			},
			"failures": 5,
		},
		"max_breaker_sec": 700,
	}
	plugin = GetPluginDefaultValues("api-breaker", plugin)

	OutputEqual(t, plugin, `{
  "healthy": {
    "http_statuses": [
      200
    ],
    "successes": 3
  },
  "max_breaker_sec": 700,
  "unhealthy": {
    "failures": 5,
    "http_statuses": [
      400, 500, 600, 700
    ]
  }
}`)

}

// Nested simple objects
func TestNestedSimpleObject(t *testing.T) {
	plugin := GetPluginDefaultValues("aws-lambda", nil)

	OutputEqual(t, plugin, `{
  "authorization": {
    "iam": {
      "aws_region": "us-east-1",
      "service": "execute-api"
    }
  },
  "keepalive": true,
  "keepalive_pool": 5,
  "keepalive_timeout": 60000,
  "ssl_verify": true,
  "timeout": 3000
}`)

	// == value overwrite ==
	plugin = Plugin{
		"authorization": map[string]interface{}{
			"iam": map[string]interface{}{
				"service": "service_name",
			},
			"some_field": "some_value",
		},
		"keepalive":      false,
		"keepalive_pool": 100,
	}
	plugin = GetPluginDefaultValues("aws-lambda", plugin)

	OutputEqual(t, plugin, `{
  "authorization": {
    "iam": {
      "aws_region": "us-east-1",
      "service": "service_name"
    },
	"some_field": "some_value"
  },
  "keepalive": false,
  "keepalive_pool": 100,
  "keepalive_timeout": 60000,
  "ssl_verify": true,
  "timeout": 3000
}`)

}

// Default and properties
func TestDefaultAndProperties(t *testing.T) {
	plugin := GetPluginDefaultValues("request-id", Plugin{
		"algorithm": "uuid",
	})

	OutputEqual(t, plugin, `{
  "algorithm": "uuid",
  "header_name": "X-Request-Id",
  "include_in_response": true,
  "range_id": {
    "char_set": "abcdefghijklmnopqrstuvwxyzABCDEFGHIGKLMNOPQRSTUVWXYZ0123456789",
    "length": 16
  }
}`)
}

// Empty Array
func TestEmptyArraySchema(t *testing.T) {
	plugin := GetPluginDefaultValues("kafka-logger", nil)

	OutputEqual(t, plugin, `{
  "batch_max_size": 1000,
  "buffer_duration": 60,
  "cluster_name": 1,
  "inactive_timeout": 5,
  "include_req_body": false,
  "include_resp_body": false,
  "max_retry_count": 0,
  "meta_format": "default",
  "meta_refresh_interval": 30,
  "name": "kafka logger",
  "producer_batch_num": 200,
  "producer_batch_size": 1048576,
  "producer_max_buffering": 50000,
  "producer_time_linger": 1,
  "producer_type": "async",
  "required_acks": 1,
  "retry_delay": 1,
  "timeout": 3
}`)

	// == value overwrite ==
	plugin = Plugin{
		"brokers": []interface{}{},
	}
	plugin = GetPluginDefaultValues("kafka-logger", plugin)

	OutputEqual(t, plugin, `{
  "batch_max_size": 1000,
  "brokers": [],
  "buffer_duration": 60,
  "cluster_name": 1,
  "inactive_timeout": 5,
  "include_req_body": false,
  "include_resp_body": false,
  "max_retry_count": 0,
  "meta_format": "default",
  "meta_refresh_interval": 30,
  "name": "kafka logger",
  "producer_batch_num": 200,
  "producer_batch_size": 1048576,
  "producer_max_buffering": 50000,
  "producer_time_linger": 1,
  "producer_type": "async",
  "required_acks": 1,
  "retry_delay": 1,
  "timeout": 3
}`)
}

// Non-empty Array
func TestArraySchema(t *testing.T) {
	// == array value overwrite ==
	plugin := Plugin{
		"brokers": []interface{}{
			// Note that we shouldn't use struct{} here. Because unmarshal only uses map[string]interface{} type
			map[string]interface{}{},
		},
	}
	plugin = GetPluginDefaultValues("kafka-logger", plugin)

	OutputEqual(t, plugin, `{
  "batch_max_size": 1000,
  "brokers": [{
    "sasl_config": {
      "mechanism": "PLAIN"
    }
  }],
  "buffer_duration": 60,
  "cluster_name": 1,
  "inactive_timeout": 5,
  "include_req_body": false,
  "include_resp_body": false,
  "max_retry_count": 0,
  "meta_format": "default",
  "meta_refresh_interval": 30,
  "name": "kafka logger",
  "producer_batch_num": 200,
  "producer_batch_size": 1048576,
  "producer_max_buffering": 50000,
  "producer_time_linger": 1,
  "producer_type": "async",
  "required_acks": 1,
  "retry_delay": 1,
  "timeout": 3
}`)

	// Make sure unmarshal also works
	plugin = Plugin{}
	err := json.Unmarshal([]byte(`{
  "brokers": [{
    "sasl_config": {
      "mechanism": "PLAIN"
    }
  }]
}`), &plugin)
	assert.Nil(t, err)
	plugin = GetPluginDefaultValues("kafka-logger", plugin)

	OutputEqual(t, plugin, `{
  "batch_max_size": 1000,
  "brokers": [{
    "sasl_config": {
      "mechanism": "PLAIN"
    }
  }],
  "buffer_duration": 60,
  "cluster_name": 1,
  "inactive_timeout": 5,
  "include_req_body": false,
  "include_resp_body": false,
  "max_retry_count": 0,
  "meta_format": "default",
  "meta_refresh_interval": 30,
  "name": "kafka logger",
  "producer_batch_num": 200,
  "producer_batch_size": 1048576,
  "producer_max_buffering": 50000,
  "producer_time_linger": 1,
  "producer_type": "async",
  "required_acks": 1,
  "retry_delay": 1,
  "timeout": 3
}`)
}

// google-cloud-logging
func TestGoogleCloudLogging(t *testing.T) {
	plugin := GetPluginDefaultValues("google-cloud-logging", nil)

	OutputEqual(t, plugin, `{
  "auth_config": {
    "entries_uri": "https://logging.googleapis.com/v2/entries:write",
    "scopes": [
      "https://www.googleapis.com/auth/logging.read",
      "https://www.googleapis.com/auth/logging.write",
      "https://www.googleapis.com/auth/logging.admin",
      "https://www.googleapis.com/auth/cloud-platform"
    ],
    "token_uri": "https://oauth2.googleapis.com/token"
  },
  "batch_max_size": 1000,
  "buffer_duration": 60,
  "inactive_timeout": 5,
  "log_id": "apisix.apache.org%2Flogs",
  "max_retry_count": 0,
  "name": "google-cloud-logging",
  "resource": {
    "type": "global"
  },
  "retry_delay": 1,
  "ssl_verify": true
}`)
}

// limit-count
func TestLimitCount(t *testing.T) {
	plugin := GetPluginDefaultValues("limit-count", nil)

	OutputEqual(t, plugin, `{
  "allow_degradation": false,
  "key": "remote_addr",
  "key_type": "var",
  "policy": "local",
  "rejected_code": 503,
  "show_limit_quota_header": true
}`)


	plugin = GetPluginDefaultValues("limit-count", Plugin{
		"policy": "redis",
	})

	OutputEqual(t, plugin, `{
  "allow_degradation": false,
  "key": "remote_addr",
  "key_type": "var",
  "policy": "redis",
  "redis_database": 0,
  "redis_port": 6379,
  "redis_ssl": false,
  "redis_ssl_verify": false,
  "redis_timeout": 1000,
  "rejected_code": 503,
  "show_limit_quota_header": true
}`)


	plugin = GetPluginDefaultValues("limit-count", Plugin{
		"policy": "redis-cluster",
	})

	OutputEqual(t, plugin, `{
  "allow_degradation": false,
  "key": "remote_addr",
  "key_type": "var",
  "policy": "redis-cluster",
  "redis_cluster_ssl": false,
  "redis_cluster_ssl_verify": false,
  "redis_timeout": 1000,
  "rejected_code": 503,
  "show_limit_quota_header": true
}`)

}

//// traffic-split
//func TestTrafficSplit(t *testing.T) {
//	plugin := GetPluginDefaultValues("traffic-split", Plugin{
//		"rule": []interface{}{
//			map[string]interface{}{
//				"match": map[string]interface{}{},
//				"weighted_upstream": map[string]interface{}{},
//			},
//		},
//	})
//
//	OutputEqual(t, plugin, ``)
//}
