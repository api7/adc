package consumer

import (
	"github.com/onsi/ginkgo/v2"
	"github.com/onsi/gomega"

	"github.com/api7/adc/pkg/api/apisix"
	"github.com/api7/adc/pkg/api/apisix/types"
	"github.com/api7/adc/test/cli/scaffold"
)

var _ = ginkgo.Describe("adc APISIX consumer SDK tests", func() {
	ginkgo.Context("Basic functions", func() {
		s := scaffold.NewScaffold()
		ginkgo.It("Consumer resource", func() {
			var (
				err      error
				consumer *types.Consumer
			)

			// utils
			assertConsumerEqual := func(expect, toBe *types.Consumer, plugins ...string) {
				gomega.Expect(expect.Username).To(gomega.Equal(toBe.Username))
				for _, plugin := range plugins {
					gomega.Expect(expect.Plugins[plugin]).NotTo(gomega.BeNil())
				}
			}

			// create consumer 1
			baseConsumer1 := &types.Consumer{
				Username: "consumer1",
			}
			_, _ = s.CreateConsumer(baseConsumer1)

			// get consumer 1
			consumer, err = s.GetConsumer("consumer1")
			gomega.Expect(err).To(gomega.BeNil())
			assertConsumerEqual(consumer, baseConsumer1)

			// create consumer 2
			baseConsumer2 := &types.Consumer{
				Username: "consumer2",
			}
			consumer, err = s.CreateConsumer(baseConsumer2)
			gomega.Expect(err).To(gomega.BeNil())
			assertConsumerEqual(consumer, baseConsumer2)

			// test list
			consumers, err := s.ListConsumer()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(len(consumers)).To(gomega.Equal(2))
			var consumer1, consumer2 *types.Consumer
			for _, r := range consumers {
				if r.Username == "consumer1" {
					consumer1 = r
				} else if r.Username == "consumer2" {
					consumer2 = r
				}
			}
			gomega.Expect(consumer1).NotTo(gomega.BeNil())
			gomega.Expect(consumer2).NotTo(gomega.BeNil())

			assertConsumerEqual(consumer1, baseConsumer1)
			assertConsumerEqual(consumer2, baseConsumer2)

			// update & get consumer 1
			baseConsumer1 = &types.Consumer{
				Username: "consumer1",
				Plugins: map[string]interface{}{
					"key-auth": map[string]interface{}{
						"key": "auth-one",
					},
				},
			}
			_, err = s.UpdateConsumer(baseConsumer1)
			gomega.Expect(err).To(gomega.BeNil())

			consumer, err = s.GetConsumer("consumer1")
			gomega.Expect(err).To(gomega.BeNil())
			assertConsumerEqual(consumer, baseConsumer1, "key-auth")

			// delete consumer 2
			err = s.DeleteConsumer("consumer2")
			gomega.Expect(err).To(gomega.BeNil())

			_, err = s.GetConsumer("consumer2")
			gomega.Expect(err).To(gomega.Equal(apisix.ErrNotFound))

			// delete consumer 1
			err = s.DeleteConsumer("consumer1")
			gomega.Expect(err).To(gomega.BeNil())

			_, err = s.GetConsumer("consumer1")
			gomega.Expect(err).To(gomega.Equal(apisix.ErrNotFound))

			// final list
			consumers, err = s.ListConsumer()
			gomega.Expect(err).To(gomega.BeNil())
			gomega.Expect(len(consumers)).To(gomega.Equal(0))

			// delete service
			err = s.DeleteService("svc1")
			gomega.Expect(err).To(gomega.BeNil())
		})
	})
})
