package validator

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"reflect"

	"github.com/api7/adc/pkg/data"
)

type Validator struct {
	localConfig *data.Configuration

	evenChan *data.Event
}

func NewValidator(local *data.Configuration) (*Validator, error) {
	return &Validator{
		localConfig: local,
	}, nil
}

type ErrorsWrapper struct {
	Errors []error
}

func (v ErrorsWrapper) Error() string {
	var errStr string
	for _, e := range v.Errors {
		errStr += e.Error()
		if !errors.Is(e, v.Errors[len(v.Errors)-1]) {
			errStr += "\n"
		}
	}
	return errStr
}

func getResourceNameOrID(resource interface{}) string {
	value := reflect.ValueOf(resource)
	value = reflect.Indirect(value)
	nameOrID := value.FieldByName("Name")
	if !nameOrID.IsValid() {
		nameOrID = value.FieldByName("ID")
	}
	return nameOrID.String()
}

func (v *Validator) validateResource(resourceType string, resource interface{}) (bool, error) {
	nameOrID := getResourceNameOrID(resource)
	errWrap := "validate resource '%s (%s)': %s"
	endpoint := fmt.Sprintf("/apisix/admin/schema/validate/%s", resourceType)
	httpClient := &http.Client{}
	jsonData, err := json.Marshal(resource)
	if err != nil {
		return false, fmt.Errorf(errWrap, resourceType, nameOrID, err)
	}

	req, err := http.NewRequest(http.MethodPost, endpoint, bytes.NewBuffer(jsonData))
	if err != nil {
		return false, fmt.Errorf(errWrap, resourceType, nameOrID, err)
	}
	resp, err := httpClient.Do(req)
	if err != nil {
		return false, fmt.Errorf(errWrap, resourceType, nameOrID, err)
	}
	defer resp.Body.Close()
	return resp.StatusCode == http.StatusOK, nil
}

func (v *Validator) Validate() []error {
	allErr := []error{}

	for _, service := range v.localConfig.Services {
		service := service
		valid, err := v.validateResource("service", service)
		if err != nil {
			allErr = append(allErr, err)
		}

		if !valid {
			// we can use event to record it or just log it
		}
	}

	return allErr
}
