package plugin_config

import (
	"reflect"

	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/pkg/api/apisix"
	"github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/test/scaffold"
)

var _ = ginkgo.Describe("adc APISIX pluginMetadata SDK tests", func() {
	ginkgo.Context("Basic functions", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("PluginMetadata resource", func() {
			var (
				err            error
				pluginMetadata *types.PluginMetadata
			)

			// utils
			assertLoggerPluginMetadataEqual := func(expect, toBe *types.PluginMetadata, plugins ...string) {
				gomega.Expect(expect.ID).To(gomega.Equal(toBe.ID))
				gomega.Expect(reflect.DeepEqual(expect.Config, toBe.Config)).To(gomega.BeTrue())
			}

			loggerConfig := map[string]interface{}{
				"log_format": map[string]interface{}{
					"host":       "$host",
					"@timestamp": "$time_iso8601",
					"client_ip":  "$remote_addr",
				},
			}

			// create http-logger
			baseHttpLogger := &types.PluginMetadata{
				ID:     "http-logger",
				Config: loggerConfig,
			}
			_, err = s.CreatePluginMetadata(baseHttpLogger)
			gomega.Expect(err).To(gomega.BeNil())

			// get http-logger
			pluginMetadata, err = s.GetPluginMetadata("http-logger")
			gomega.Expect(err).To(gomega.BeNil())
			assertLoggerPluginMetadataEqual(pluginMetadata, baseHttpLogger, "limit-count")

			// create skywalking-logger
			baseSkywalkingLogger := &types.PluginMetadata{
				ID:     "skywalking-logger",
				Config: loggerConfig,
			}
			pluginMetadata, err = s.CreatePluginMetadata(baseSkywalkingLogger)
			gomega.Expect(err).To(gomega.BeNil())
			assertLoggerPluginMetadataEqual(pluginMetadata, baseSkywalkingLogger, "limit-count")

			// test list
			pluginMetadatas, err := s.ListPluginMetadata()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(len(pluginMetadatas)).To(gomega.Equal(2))
			var pluginMetadata1, pluginMetadata2 *types.PluginMetadata
			for _, r := range pluginMetadatas {
				if r.ID == "http-logger" {
					pluginMetadata1 = r
				} else if r.ID == "skywalking-logger" {
					pluginMetadata2 = r
				}
			}
			gomega.Expect(pluginMetadata1).NotTo(gomega.BeNil())
			gomega.Expect(pluginMetadata2).NotTo(gomega.BeNil())

			assertLoggerPluginMetadataEqual(pluginMetadata1, baseHttpLogger, "limit-count")
			assertLoggerPluginMetadataEqual(pluginMetadata2, baseSkywalkingLogger, "limit-count")

			// update & get http-logger
			baseHttpLogger = &types.PluginMetadata{
				ID: "http-logger",
				Config: map[string]interface{}{
					"log_format": map[string]interface{}{
						"host":       "$host",
						"@timestamp": "$any_changed_value",
						"client_ip":  "$remote_addr",
					},
				},
			}
			_, err = s.UpdatePluginMetadata(baseHttpLogger)
			gomega.Expect(err).To(gomega.BeNil())

			pluginMetadata, err = s.GetPluginMetadata("http-logger")
			gomega.Expect(err).To(gomega.BeNil())
			assertLoggerPluginMetadataEqual(pluginMetadata, baseHttpLogger, "key-auth")

			// delete skywalking-logger
			err = s.DeletePluginMetadata("skywalking-logger")
			gomega.Expect(err).To(gomega.BeNil())

			_, err = s.GetPluginMetadata("skywalking-logger")
			gomega.Expect(err).To(gomega.Equal(apisix.ErrNotFound))

			// delete http-logger
			err = s.DeletePluginMetadata("http-logger")
			gomega.Expect(err).To(gomega.BeNil())

			_, err = s.GetPluginMetadata("http-logger")
			gomega.Expect(err).To(gomega.Equal(apisix.ErrNotFound))

			// final list
			pluginMetadatas, err = s.ListPluginMetadata()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(len(pluginMetadatas)).To(gomega.Equal(0))

			// delete service
			err = s.DeleteService("svc1")
			gomega.Expect(err).To(gomega.BeNil())
		})
	})
})
