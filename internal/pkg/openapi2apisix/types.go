package openapi2apisix

import (
	"context"

	"github.com/api7/gopkg/pkg/log"
	"github.com/getkin/kin-openapi/openapi3"
	"github.com/pkg/errors"
	"go.uber.org/zap"

	apitypes "github.com/api7/adc/pkg/api/apisix/types"
)

var (
	// LoadOpenAPIError  is the error type for loading OpenAPI
	LoadOpenAPIError = errors.New("load OpenAPI file error")
)

type ResultBundle struct {
	Services []*apitypes.Service `json:"service"`
	Routes   []*apitypes.Route   `json:"routes"`
}

type OAS []byte

func (s OAS) LoadOpenAPI(ctx context.Context) (*openapi3.T, error) {
	doc, err := openapi3.NewLoader().LoadFromData(s)
	if err != nil {
		log.Warnw("load OpenAPI error", zap.Error(err))
		return nil, errors.New("failed to load OpenAPI")
	}
	if doc.OpenAPI == "" {
		return nil, errors.New("the file you uploaded is not an OpenAPI document")
	}
	if doc.Info == nil {
		return nil, errors.New("the file you uploaded is not a valid OpenAPI document")
	}
	// now we only care about info and paths in openapi
	err = doc.Info.Validate(
		ctx,
		openapi3.DisableSchemaPatternValidation(),
		openapi3.DisableSchemaDefaultsValidation(),
		openapi3.DisableExamplesValidation(),
	)
	if err != nil {
		return nil, errors.Wrap(err, "the file you uploaded is not a valid OpenAPI document")
	}
	if doc.Paths != nil {
		err = doc.Paths.Validate(
			ctx,
			openapi3.DisableSchemaPatternValidation(),
			openapi3.DisableSchemaDefaultsValidation(),
			openapi3.DisableExamplesValidation(),
		)
		if err != nil {
			return nil, errors.Wrap(err, "the file you uploaded is not a valid OpenAPI document")
		}
	}

	return doc, nil
}
