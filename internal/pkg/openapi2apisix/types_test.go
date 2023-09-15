package openapi2apisix

import (
	"context"
	"testing"

	"github.com/getkin/kin-openapi/openapi3"
	"github.com/stretchr/testify/assert"
)

func TestOAS_LoadOpenAPI(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name         string
		oas          OAS
		want         *openapi3.T
		pathItemKeys []string
		errorReason  string
	}{
		{
			name:        "invalid openapi",
			oas:         OAS(`error`),
			want:        nil,
			errorReason: "failed to load OpenAPI",
		},
		{
			name:        "openapi is empty",
			oas:         OAS(`{"openapi": ""}`),
			want:        nil,
			errorReason: "the file you uploaded is not an OpenAPI document",
		},
		{
			name:        "info is nil",
			oas:         OAS(`{"openapi": "3.0.0"}`),
			want:        nil,
			errorReason: "the file you uploaded is not a valid OpenAPI document",
		},
		{
			name: "success",
			oas:  tagsJsonTest,
			want: &openapi3.T{
				OpenAPI: "3.0.0",
				Info: &openapi3.Info{
					Title:       "API 101",
					Description: "modify operationId",
					Version:     "1.0.0",
				},
				Paths: openapi3.Paths{
					"/customers": {
						Get: &openapi3.Operation{
							Tags:        []string{"default", "customer"},
							Summary:     "Get all customers",
							OperationID: "getCustomers",
						},
					},
				},
			},
			pathItemKeys: []string{"/customers"},
			errorReason:  "",
		},
	}
	for _, tt := range tests {
		tt := tt
		t.Run(tt.name, func(t *testing.T) {
			got, err := tt.oas.LoadOpenAPI(context.TODO())
			if tt.errorReason != "" {
				assert.NotNil(t, err)
				assert.Contains(t, err.Error(), tt.errorReason)
			} else {
				assert.Nil(t, err)
				assert.Equal(t, tt.want.OpenAPI, got.OpenAPI)
				assert.Equal(t, tt.want.Info.Title, got.Info.Title)
				assert.Equal(t, tt.want.Info.Description, got.Info.Description)
				assert.Equal(t, tt.want.Info.Version, got.Info.Version)
				if tt.want.Paths != nil {
					for _, key := range tt.pathItemKeys {
						assert.NotNil(t, got.Paths[key], "%s should not be nil", key)
					}
				}
			}
		})
	}
}
