package openapi2apisix

import (
	"context"
	_ "embed"
	"testing"

	"github.com/stretchr/testify/assert"

	apitypes "github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/pkg/common"
)

func TestSlugify(t *testing.T) {
	type args struct {
		name []string
	}
	tests := []struct {
		name string
		args args
		want string
	}{
		{
			name: "one",
			args: args{
				name: []string{"Pet Store"},
			},
			want: "pet-store",
		},
		{
			name: "path1",
			args: args{
				name: []string{"GET", "/api/services"},
			},
			want: "get_api-services",
		},
		{
			name: "path2",
			args: args{
				name: []string{"POST", "/api/services"},
			},
			want: "post_api-services",
		},
		{
			name: "path3",
			args: args{
				name: []string{"PUT", "/api/services/{service_id}"},
			},
			want: "put_api-services-service-id",
		},
		{
			name: "path4",
			args: args{
				name: []string{"DELETE", "/api/services/{service_id}"},
			},
			want: "delete_api-services-service-id",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := Slugify(tt.args.name...); got != tt.want {
				t.Errorf("Slugify() = %v, want %v", got, tt.want)
			}
		})
	}
}

var (
	//go:embed testdata/Postman-API101.yaml
	postman101 []byte
	//go:embed testdata/operationId.yaml
	operationIdTest []byte
	//go:embed testdata/tags.yaml
	tagsTest []byte
	//go:embed testdata/tags.json
	tagsJsonTest []byte
	//go:embed testdata/urlVariables.yaml
	urlVariables []byte
)

func TestConvert(t *testing.T) {
	type args struct {
		content []byte
	}
	tests := []struct {
		name    string
		args    args
		want    *apitypes.Configuration
		wantErr bool
	}{
		{
			name: "postman101",
			args: args{
				content: postman101,
			},
			want: &apitypes.Configuration{
				Services: []*apitypes.Service{
					{
						Name:        "API 101",
						Description: `API 101 template for learning API request basics. Follow along with the webinar / video or just open the first request and hit **Send**!`,
						Upstream: &apitypes.Upstream{
							Scheme: "https",
							Nodes: []apitypes.UpstreamNode{
								{
									Host:   "api-101.glitch.me",
									Port:   443,
									Weight: 100,
								},
								{
									Host:   "{{apiurl}}",
									Port:   80,
									Weight: 100,
								},
							},
							Timeout: &apitypes.UpstreamTimeout{
								Connect: 60,
								Send:    60,
								Read:    60,
							},
							PassHost: apitypes.UpstreamPassHostPass,
						},
						//  Labels:      make([]apitypes.Labels, 0),
					},
				},
				Routes: []*apitypes.Route{
					{
						Name:        "api-101_get_customer",
						Description: "Get one customer",
						// Labels:      []string{"default"},
						Methods: []string{"GET"},
						Uris:    []string{"/customer"},
					},
					{
						Name:        "api-101_post_customer",
						Description: "Add new customer",
						// Labels:      []string{"default"},
						Methods: []string{"POST"},
						Uris:    []string{"/customer"},
					},
					{
						Name:        "api-101_delete_customer-customer-id",
						Description: "Remove customer",
						// Labels:      []string{"default"},
						Methods: []string{"DELETE"},
						Uris:    []string{"/customer/:customer_id"},
					},
					{
						Name:        "api-101_put_customer-customer-id",
						Description: "Update customer",
						// Labels:      []string{"default"},
						Methods: []string{"PUT"},
						Uris:    []string{"/customer/:customer_id"},
					},
					{
						Name:        "api-101_get_customers",
						Description: "Get all customers",
						// Labels:      []string{"default"},
						Methods: []string{"GET"},
						Uris:    []string{"/customers"},
					},
				},
			},
			wantErr: false,
		},
		{
			name: "operationId",
			args: args{
				content: operationIdTest,
			},
			want: &apitypes.Configuration{
				Services: []*apitypes.Service{
					{
						Name:        "API 101",
						Description: "modify operationId",
						Upstream: &apitypes.Upstream{
							Scheme: "https",
							Nodes: []apitypes.UpstreamNode{
								{
									Host:   "api-101.glitch.me",
									Port:   443,
									Weight: 100,
								},
							},
							Timeout: &apitypes.UpstreamTimeout{
								Connect: 60,
								Send:    60,
								Read:    60,
							},
							PassHost: apitypes.UpstreamPassHostPass,
						},
						// Labels:        make([]apitypes.Labels, 0),
					},
				},
				Routes: []*apitypes.Route{
					{
						Name:        "update Customer",
						Description: "Update customer",
						// Labels:      []string{"default"},
						Methods: []string{"PUT"},
						Uris:    []string{"/customer/:customer_id"},
					},
					{
						Name:        "getCustomers",
						Description: "Get all customers",
						// Labels:      []string{"default"},
						Methods: []string{"GET"},
						Uris:    []string{"/customers"},
					},
				},
			},
			wantErr: false,
		},
		{
			name: "tags",
			args: args{
				content: tagsTest,
			},
			want: &apitypes.Configuration{
				Services: []*apitypes.Service{
					{
						Name:        "API 101",
						Description: "modify operationId",
						Upstream: &apitypes.Upstream{
							Scheme: "https",
							Nodes: []apitypes.UpstreamNode{
								{
									Host:   "api-101.glitch.me",
									Port:   443,
									Weight: 100,
								},
							},
							Timeout: &apitypes.UpstreamTimeout{
								Connect: 60,
								Send:    60,
								Read:    60,
							},
							PassHost: apitypes.UpstreamPassHostPass,
						},
						// Labels: apitypes.Labels{
						//		"web-spider", "blockchain",
						//	},
					},
				},
				Routes: []*apitypes.Route{
					{
						Name:        "getCustomers",
						Description: "Get all customers",
						// Labels:      []string{"default", "customer"},
						Methods: []string{"GET"},
						Uris:    []string{"/customers"},
					},
				},
			},
			wantErr: false,
		},
		{
			name: "tags -> json file",
			args: args{
				content: tagsJsonTest,
			},
			want: &apitypes.Configuration{
				Services: []*apitypes.Service{
					{
						Name:        "API 101",
						Description: "modify operationId",
						Upstream: &apitypes.Upstream{
							Scheme: "https",
							Nodes: []apitypes.UpstreamNode{
								{
									Host:   "api-101.glitch.me",
									Port:   443,
									Weight: 100,
								},
							},
							Timeout: &apitypes.UpstreamTimeout{
								Connect: 60,
								Send:    60,
								Read:    60,
							},
							PassHost: apitypes.UpstreamPassHostPass,
						},
						// Labels: []apitypes.Labels{
						// "web-spider", "blockchain",
						//},
					},
				},
				Routes: []*apitypes.Route{
					{
						Name:        "getCustomers",
						Description: "Get all customers",
						// Labels:      []string{"default", "customer"},
						Methods: []string{"GET"},
						Uris:    []string{"/customers"},
					},
				},
			},
			wantErr: false,
		},
		{
			name: "urlVariables",
			args: args{
				content: urlVariables,
			},
			want: &apitypes.Configuration{
				Services: []*apitypes.Service{
					{
						Name: "URL variables",
						Upstream: &apitypes.Upstream{
							Scheme: "https",
							Nodes: []apitypes.UpstreamNode{
								{
									Host:   "example.com",
									Port:   443,
									Weight: 100,
								},
							},
							Timeout: &apitypes.UpstreamTimeout{
								Connect: 60,
								Send:    60,
								Read:    60,
							},
							PassHost: apitypes.UpstreamPassHostPass,
						},
						//  Labels:      make([]apitypes.Labels, 0),
					},
				},
				Routes: []*apitypes.Route{
					{
						Name: "url-variables_get_base64-value",
						// Labels:      []string{"default"},
						Methods: []string{"GET"},
						Uris:    []string{"/base64/:value"},
					},
					{
						Name: "url-variables_get_basic-auth-user-passwd",
						// Labels:      []string{"default"},
						Methods: []string{"GET"},
						Uris:    []string{"/basic-auth/:user/:passwd"},
					},
					{
						Name: "url-variables_get_digest-auth-qop-user-passwd-algorithm-stale-after",
						// Labels:      []string{"default"},
						Methods: []string{"GET"},
						Uris:    []string{"/digest-auth/:qop/:user/:passwd/:algorithm/:stale_after"},
					},
				},
			},
			wantErr: false,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := Convert(context.TODO(), tt.args.content)
			if (err != nil) != tt.wantErr {
				t.Errorf("Convert() error = %v, wantErr %v", err, tt.wantErr)
				return
			}
			if got != nil {
				// normally the converter should only generate one service
				svc := tt.want.Services[0]
				svc.ID = common.GenID(svc.Name)
				if svc.Upstream != nil {
					if svc.Upstream.Nodes == nil {
						svc.Upstream.Nodes = []apitypes.UpstreamNode{
							{},
						}
					}
					svc.Upstream.Name = Slugify("Upstream for", svc.Name)
					svc.Upstream.ID = common.GenID(svc.Upstream.Name)
				}

				for _, route := range tt.want.Routes {
					route.ID = common.GenID(route.Name)
					route.ServiceID = svc.ID
				}

				assert.Equal(t, tt.want.Services, got.Services)
				assert.Equal(t, tt.want.Routes, got.Routes)
			}
		})
	}
}
