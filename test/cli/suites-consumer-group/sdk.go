package consumer_group

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/pkg/api/apisix"
	"github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/test/cli/scaffold"
)

var _ = ginkgo.Describe("adc APISIX consumerGroup SDK tests", func() {
	ginkgo.Context("Basic functions", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("ConsumerGroup resource", func() {
			var (
				err          error
				consumerGroup *types.ConsumerGroup
			)

			// utils
			assertConsumerGroupEqual := func(expect, toBe *types.ConsumerGroup, plugins ...string) {
				gomega.Expect(expect.ID).To(gomega.Equal(toBe.ID))
				gomega.Expect(expect.Desc).To(gomega.Equal(toBe.Desc))
				for _, plugin := range plugins {
					gomega.Expect(expect.Plugins[plugin]).NotTo(gomega.BeNil())
				}
			}

			// create consumerGroup 1
			baseConsumerGroup1 := &types.ConsumerGroup{
				ID: "consumerGroup1",
				Plugins: map[string]interface{}{
					"limit-count": map[string]interface{}{
						"time_window":   60,
						"policy":        "local",
						"count":         100,
						"key":           "remote_addr",
						"rejected_code": 503,
					},
				},
			}
			_, err = s.CreateConsumerGroup(baseConsumerGroup1)

			// get consumerGroup 1
			consumerGroup, err = s.GetConsumerGroup("consumerGroup1")
			gomega.Expect(err).To(gomega.BeNil())
			assertConsumerGroupEqual(consumerGroup, baseConsumerGroup1, "limit-count")

			// create consumerGroup 2
			baseConsumerGroup2 := &types.ConsumerGroup{
				ID: "consumerGroup2",
				Plugins: map[string]interface{}{
					"limit-count": map[string]interface{}{
						"time_window":   60,
						"policy":        "local",
						"count":         200,
						"key":           "remote_addr",
						"rejected_code": 503,
					},
				},
			}
			consumerGroup, err = s.CreateConsumerGroup(baseConsumerGroup2)
			gomega.Expect(err).To(gomega.BeNil())
			assertConsumerGroupEqual(consumerGroup, baseConsumerGroup2, "limit-count")

			// test list
			consumerGroups, err := s.ListConsumerGroup()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(len(consumerGroups)).To(gomega.Equal(2))
			var consumerGroup1, consumerGroup2 *types.ConsumerGroup
			for _, r := range consumerGroups {
				if r.ID == "consumerGroup1" {
					consumerGroup1 = r
				} else if r.ID == "consumerGroup2" {
					consumerGroup2 = r
				}
			}
			gomega.Expect(consumerGroup1).NotTo(gomega.BeNil())
			gomega.Expect(consumerGroup2).NotTo(gomega.BeNil())

			assertConsumerGroupEqual(consumerGroup1, baseConsumerGroup1, "limit-count")
			assertConsumerGroupEqual(consumerGroup2, baseConsumerGroup2, "limit-count")

			// update & get consumerGroup 1
			baseConsumerGroup1 = &types.ConsumerGroup{
				ID: "consumerGroup1",
				Plugins: map[string]interface{}{
					"key-auth": map[string]interface{}{
						"key": "auth-one",
					},
				},
			}
			_, err = s.UpdateConsumerGroup(baseConsumerGroup1)
			gomega.Expect(err).To(gomega.BeNil())

			consumerGroup, err = s.GetConsumerGroup("consumerGroup1")
			gomega.Expect(err).To(gomega.BeNil())
			assertConsumerGroupEqual(consumerGroup, baseConsumerGroup1, "key-auth")

			// delete consumerGroup 2
			err = s.DeleteConsumerGroup("consumerGroup2")
			gomega.Expect(err).To(gomega.BeNil())

			_, err = s.GetConsumerGroup("consumerGroup2")
			gomega.Expect(err).To(gomega.Equal(apisix.ErrNotFound))

			// delete consumerGroup 1
			err = s.DeleteConsumerGroup("consumerGroup1")
			gomega.Expect(err).To(gomega.BeNil())

			_, err = s.GetConsumerGroup("consumerGroup1")
			gomega.Expect(err).To(gomega.Equal(apisix.ErrNotFound))

			// final list
			consumerGroups, err = s.ListConsumerGroup()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(len(consumerGroups)).To(gomega.Equal(0))

			// delete service
			err = s.DeleteService("svc1")
			gomega.Expect(err).To(gomega.BeNil())
		})
	})
})
