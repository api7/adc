package plugin_config

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/pkg/api/apisix"
	"github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/test/scaffold"
)

var _ = ginkgo.Describe("adc APISIX pluginConfig SDK tests", func() {
	ginkgo.Context("Basic functions", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("PluginConfig resource", func() {
			var (
				err          error
				pluginConfig *types.PluginConfig
			)

			// utils
			assertPluginConfigEqual := func(expect, toBe *types.PluginConfig, plugins ...string) {
				gomega.Expect(expect.ID).To(gomega.Equal(toBe.ID))
				gomega.Expect(expect.Desc).To(gomega.Equal(toBe.Desc))
				for _, plugin := range plugins {
					gomega.Expect(expect.Plugins[plugin]).NotTo(gomega.BeNil())
				}
			}

			// create pluginConfig 1
			basePluginConfig1 := &types.PluginConfig{
				ID: "pluginConfig1",
				Plugins: types.Plugins{
					"limit-count": types.Plugin{
						"time_window":   60,
						"policy":        "local",
						"count":         100,
						"key":           "remote_addr",
						"rejected_code": 503,
					},
				},
			}
			_, err = s.CreatePluginConfig(basePluginConfig1)
			gomega.Expect(err).To(gomega.BeNil())

			// get pluginConfig 1
			pluginConfig, err = s.GetPluginConfig("pluginConfig1")
			gomega.Expect(err).To(gomega.BeNil())
			assertPluginConfigEqual(pluginConfig, basePluginConfig1, "limit-count")

			// create pluginConfig 2
			basePluginConfig2 := &types.PluginConfig{
				ID: "pluginConfig2",
				Plugins: types.Plugins{
					"limit-count": types.Plugin{
						"time_window":   60,
						"policy":        "local",
						"count":         200,
						"key":           "remote_addr",
						"rejected_code": 503,
					},
				},
			}
			pluginConfig, err = s.CreatePluginConfig(basePluginConfig2)
			gomega.Expect(err).To(gomega.BeNil())
			assertPluginConfigEqual(pluginConfig, basePluginConfig2, "limit-count")

			// test list
			pluginConfigs, err := s.ListPluginConfig()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(len(pluginConfigs)).To(gomega.Equal(2))
			var pluginConfig1, pluginConfig2 *types.PluginConfig
			for _, r := range pluginConfigs {
				if r.ID == "pluginConfig1" {
					pluginConfig1 = r
				} else if r.ID == "pluginConfig2" {
					pluginConfig2 = r
				}
			}
			gomega.Expect(pluginConfig1).NotTo(gomega.BeNil())
			gomega.Expect(pluginConfig2).NotTo(gomega.BeNil())

			assertPluginConfigEqual(pluginConfig1, basePluginConfig1, "limit-count")
			assertPluginConfigEqual(pluginConfig2, basePluginConfig2, "limit-count")

			// update & get pluginConfig 1
			basePluginConfig1 = &types.PluginConfig{
				ID: "pluginConfig1",
				Plugins: types.Plugins{
					"key-auth": types.Plugin{
						"key": "auth-one",
					},
				},
			}
			_, err = s.UpdatePluginConfig(basePluginConfig1)
			gomega.Expect(err).To(gomega.BeNil())

			pluginConfig, err = s.GetPluginConfig("pluginConfig1")
			gomega.Expect(err).To(gomega.BeNil())
			assertPluginConfigEqual(pluginConfig, basePluginConfig1, "key-auth")

			// delete pluginConfig 2
			err = s.DeletePluginConfig("pluginConfig2")
			gomega.Expect(err).To(gomega.BeNil())

			_, err = s.GetPluginConfig("pluginConfig2")
			gomega.Expect(err).To(gomega.Equal(apisix.ErrNotFound))

			// delete pluginConfig 1
			err = s.DeletePluginConfig("pluginConfig1")
			gomega.Expect(err).To(gomega.BeNil())

			_, err = s.GetPluginConfig("pluginConfig1")
			gomega.Expect(err).To(gomega.Equal(apisix.ErrNotFound))

			// final list
			pluginConfigs, err = s.ListPluginConfig()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(len(pluginConfigs)).To(gomega.Equal(0))

			// delete service
			err = s.DeleteService("svc1")
			gomega.Expect(err).To(gomega.BeNil())
		})
	})
})
