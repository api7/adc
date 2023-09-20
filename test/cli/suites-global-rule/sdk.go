package global_rule

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/pkg/api/apisix"
	"github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/test/cli/scaffold"
)

var _ = ginkgo.Describe("adc APISIX globalRule SDK tests", func() {
	ginkgo.Context("Basic functions", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("GlobalRule resource", func() {
			var (
				err        error
				globalRule *types.GlobalRule
			)

			// utils
			assertGlobalRuleEqual := func(expect, toBe *types.GlobalRule, plugins ...string) {
				gomega.Expect(expect.ID).To(gomega.Equal(toBe.ID))
				for _, plugin := range plugins {
					gomega.Expect(expect.Plugins[plugin]).NotTo(gomega.BeNil())
				}
			}

			// create globalRule 1
			baseGlobalRule1 := &types.GlobalRule{
				ID: "globalRule1",
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
			_, err = s.CreateGlobalRule(baseGlobalRule1)

			// get globalRule 1
			globalRule, err = s.GetGlobalRule("globalRule1")
			gomega.Expect(err).To(gomega.BeNil())
			assertGlobalRuleEqual(globalRule, baseGlobalRule1, "limit-count")

			// create globalRule 2
			baseGlobalRule2 := &types.GlobalRule{
				ID: "globalRule2",
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
			globalRule, err = s.CreateGlobalRule(baseGlobalRule2)
			gomega.Expect(err).To(gomega.BeNil())
			assertGlobalRuleEqual(globalRule, baseGlobalRule2, "limit-count")

			// test list
			globalRules, err := s.ListGlobalRule()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(len(globalRules)).To(gomega.Equal(2))
			var globalRule1, globalRule2 *types.GlobalRule
			for _, r := range globalRules {
				if r.ID == "globalRule1" {
					globalRule1 = r
				} else if r.ID == "globalRule2" {
					globalRule2 = r
				}
			}
			gomega.Expect(globalRule1).NotTo(gomega.BeNil())
			gomega.Expect(globalRule2).NotTo(gomega.BeNil())

			assertGlobalRuleEqual(globalRule1, baseGlobalRule1, "limit-count")
			assertGlobalRuleEqual(globalRule2, baseGlobalRule2, "limit-count")

			// update & get globalRule 1
			baseGlobalRule1 = &types.GlobalRule{
				ID: "globalRule1",
				Plugins: types.Plugins{
					"key-auth": types.Plugin{
						"key": "auth-one",
					},
				},
			}
			_, err = s.UpdateGlobalRule(baseGlobalRule1)
			gomega.Expect(err).To(gomega.BeNil())

			globalRule, err = s.GetGlobalRule("globalRule1")
			gomega.Expect(err).To(gomega.BeNil())
			assertGlobalRuleEqual(globalRule, baseGlobalRule1, "key-auth")

			// delete globalRule 2
			err = s.DeleteGlobalRule("globalRule2")
			gomega.Expect(err).To(gomega.BeNil())

			_, err = s.GetGlobalRule("globalRule2")
			gomega.Expect(err).To(gomega.Equal(apisix.ErrNotFound))

			// delete globalRule 1
			err = s.DeleteGlobalRule("globalRule1")
			gomega.Expect(err).To(gomega.BeNil())

			_, err = s.GetGlobalRule("globalRule1")
			gomega.Expect(err).To(gomega.Equal(apisix.ErrNotFound))

			// final list
			globalRules, err = s.ListGlobalRule()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(len(globalRules)).To(gomega.Equal(0))

			// delete service
			err = s.DeleteService("svc1")
			gomega.Expect(err).To(gomega.BeNil())
		})
	})
})
