import * as ADCSDK from '@api7/adc-sdk';
import { randomUUID } from 'crypto';
import { diff as objectDiff } from 'deep-diff';
import { cloneDeep, has, isEmpty, isEqual, isNil, unset } from 'lodash';

const order = {
  [`${ADCSDK.ResourceType.ROUTE}.${ADCSDK.EventType.DELETE}`]: 0,
  [`${ADCSDK.ResourceType.STREAM_ROUTE}.${ADCSDK.EventType.DELETE}`]: 1,
  [`${ADCSDK.ResourceType.SERVICE}.${ADCSDK.EventType.DELETE}`]: 2,
  [`${ADCSDK.ResourceType.UPSTREAM}.${ADCSDK.EventType.DELETE}`]: 3,
  [`${ADCSDK.ResourceType.PLUGIN_CONFIG}.${ADCSDK.EventType.DELETE}`]: 4,
  [`${ADCSDK.ResourceType.CONSUMER}.${ADCSDK.EventType.DELETE}`]: 5,
  [`${ADCSDK.ResourceType.CONSUMER_GROUP}.${ADCSDK.EventType.DELETE}`]: 6,
  [`${ADCSDK.ResourceType.SSL}.${ADCSDK.EventType.DELETE}`]: 7,

  [`${ADCSDK.ResourceType.ROUTE}.${ADCSDK.EventType.UPDATE}`]: 8,
  [`${ADCSDK.ResourceType.STREAM_ROUTE}.${ADCSDK.EventType.UPDATE}`]: 9,
  [`${ADCSDK.ResourceType.SERVICE}.${ADCSDK.EventType.UPDATE}`]: 10,
  [`${ADCSDK.ResourceType.UPSTREAM}.${ADCSDK.EventType.UPDATE}`]: 11,
  [`${ADCSDK.ResourceType.PLUGIN_CONFIG}.${ADCSDK.EventType.UPDATE}`]: 12,
  [`${ADCSDK.ResourceType.CONSUMER_GROUP}.${ADCSDK.EventType.UPDATE}`]: 13,
  [`${ADCSDK.ResourceType.CONSUMER}.${ADCSDK.EventType.UPDATE}`]: 14,
  [`${ADCSDK.ResourceType.SSL}.${ADCSDK.EventType.UPDATE}`]: 15,

  [`${ADCSDK.ResourceType.SSL}.${ADCSDK.EventType.CREATE}`]: 16, // SSL may be referenced by upstream mTLS, so it needs to be created in advance
  [`${ADCSDK.ResourceType.SERVICE}.${ADCSDK.EventType.CREATE}`]: 17,
  [`${ADCSDK.ResourceType.PLUGIN_CONFIG}.${ADCSDK.EventType.CREATE}`]: 18,
  [`${ADCSDK.ResourceType.ROUTE}.${ADCSDK.EventType.CREATE}`]: 19,
  [`${ADCSDK.ResourceType.STREAM_ROUTE}.${ADCSDK.EventType.CREATE}`]: 20,
  [`${ADCSDK.ResourceType.CONSUMER_GROUP}.${ADCSDK.EventType.CREATE}`]: 21,
  [`${ADCSDK.ResourceType.CONSUMER}.${ADCSDK.EventType.CREATE}`]: 22,

  [`${ADCSDK.ResourceType.UPSTREAM}.${ADCSDK.EventType.CREATE}`]: 23,
  [`${ADCSDK.ResourceType.GLOBAL_RULE}.${ADCSDK.EventType.DELETE}`]: 24,
  [`${ADCSDK.ResourceType.GLOBAL_RULE}.${ADCSDK.EventType.CREATE}`]: 25,
  [`${ADCSDK.ResourceType.GLOBAL_RULE}.${ADCSDK.EventType.UPDATE}`]: 26,
  [`${ADCSDK.ResourceType.PLUGIN_METADATA}.${ADCSDK.EventType.DELETE}`]: 27,
  [`${ADCSDK.ResourceType.PLUGIN_METADATA}.${ADCSDK.EventType.CREATE}`]: 28,
  [`${ADCSDK.ResourceType.PLUGIN_METADATA}.${ADCSDK.EventType.UPDATE}`]: 29,
  [`${ADCSDK.ResourceType.CONSUMER_CREDENTIAL}.${ADCSDK.EventType.DELETE}`]: 30,
  [`${ADCSDK.ResourceType.CONSUMER_CREDENTIAL}.${ADCSDK.EventType.CREATE}`]: 31,
  [`${ADCSDK.ResourceType.CONSUMER_CREDENTIAL}.${ADCSDK.EventType.UPDATE}`]: 32,
};

const showLogEntry = () =>
  ['true', '1'].includes(process?.env?.ADC_DIFFER_DEBUG ?? '');

export class DifferV3 {
  private readonly defaultValue: ADCSDK.DefaultValue;
  private readonly transactionId: string;
  private readonly logger?: ADCSDK.Logger;

  constructor(opts: {
    transactionId: string;
    defaultValue: ADCSDK.DefaultValue;
    logger?: ADCSDK.Logger;
  }) {
    this.transactionId = opts.transactionId;
    this.defaultValue = opts.defaultValue;
    this.logger = opts.logger;
  }

  public static diff(
    local: ADCSDK.Configuration,
    remote: ADCSDK.Configuration,
    defaultValue?: ADCSDK.DefaultValue,
    parentName?: string,
    logger?: ADCSDK.Logger,
  ): Array<ADCSDK.Event> {
    const differ = new DifferV3({
      transactionId: randomUUID(),
      defaultValue,
      logger,
    });

    differ.logger?.debug(
      {
        message: 'Enter differ',
        transactionId: differ.transactionId,
        local,
        remote,
        defaultValue,
      },
      { showLogEntry },
    );

    const generateResourceName = (name: string) =>
      parentName ? `${parentName}.${name}` : name;
    const result = [
      ...differ.diffResource(
        ADCSDK.ResourceType.SERVICE,
        local?.services?.map((res) => [
          res.name,
          res.id ?? ADCSDK.utils.generateId(res.name),
          res,
        ]) ?? [],
        remote?.services?.map((res) => [res.name, res.id, res]) ?? [],
      ),
      ...differ.diffResource(
        ADCSDK.ResourceType.SSL,
        local?.ssls?.map((res) => [
          res.snis.join(','),
          res.id ?? ADCSDK.utils.generateId(res.snis.join(',')),
          res,
        ]) ?? [],
        remote?.ssls?.map((res) => [res.snis.join(','), res.id, res]) ?? [],
      ),
      ...differ.diffResource(
        ADCSDK.ResourceType.CONSUMER,
        local?.consumers?.map((res) => [res.username, res.username, res]) ?? [],
        remote?.consumers?.map((res) => [res.username, res.username, res]) ??
          [],
      ),
      ...differ.diffResource(
        ADCSDK.ResourceType.GLOBAL_RULE,
        Object.entries(local?.global_rules ?? {}).map(
          ([pluginName, pluginConfig]) => [
            pluginName,
            pluginName,
            pluginConfig,
          ],
        ) ?? [],
        Object.entries(remote?.global_rules ?? {}).map(
          ([pluginName, pluginConfig]) => [
            pluginName,
            pluginName,
            pluginConfig,
          ],
        ) ?? [],
      ),
      ...differ.diffResource(
        ADCSDK.ResourceType.PLUGIN_METADATA,
        Object.entries(local?.plugin_metadata ?? {}).map(
          ([pluginName, pluginConfig]) => [
            pluginName,
            pluginName,
            pluginConfig,
          ],
        ) ?? [],
        Object.entries(remote?.plugin_metadata ?? {}).map(
          ([pluginName, pluginConfig]) => [
            pluginName,
            pluginName,
            pluginConfig,
          ],
        ) ?? [],
      ),
      ...differ.diffResource(
        ADCSDK.ResourceType.ROUTE,
        local?.routes?.map((res) => [
          res.name,
          res.id ?? ADCSDK.utils.generateId(generateResourceName(res.name)),
          res,
        ]) ?? [],
        remote?.routes?.map((res) => [res.name, res.id, res]) ?? [],
      ),
      ...differ.diffResource(
        ADCSDK.ResourceType.STREAM_ROUTE,
        local?.stream_routes?.map((res) => [
          res.name,
          res.id ?? ADCSDK.utils.generateId(generateResourceName(res.name)),
          res,
        ]) ?? [],
        remote?.stream_routes?.map((res) => [res.name, res.id, res]) ?? [],
      ),
      ...differ.diffResource(
        ADCSDK.ResourceType.CONSUMER_CREDENTIAL,
        local?.consumer_credentials?.map((res) => [
          res.name,
          res.id ?? ADCSDK.utils.generateId(generateResourceName(res.name)),
          res,
        ]) ?? [],
        remote?.consumer_credentials?.map((res) => [res.name, res.id, res]) ??
          [],
      ),
      ...differ.diffResource(
        ADCSDK.ResourceType.UPSTREAM,
        local?.upstreams?.map((res) => [
          res.name,
          res.id ?? ADCSDK.utils.generateId(generateResourceName(res.name)),
          res,
        ]) ?? [],
        remote?.upstreams?.map((res) => [res.name, res.id, res]) ?? [],
      ),
      /* ...differ.diffResource(
        ADCSDK.ResourceType.CONSUMER_GROUP,
        local?.consumer_groups?.map((res) => [
          res.name,
          ADCSDK.utils.generateId(res.name),
          res,
        ]) ?? [],
        remote?.consumer_groups?.map((res) => [
          res.name,
          ADCSDK.utils.generateId(res.name),
          res,
        ]) ?? [],
      ),
      ...differ.diffResource(
        ADCSDK.ResourceType.PLUGIN_CONFIG,
        local?.plugin_configs?.map((res) => [
          res.name,
          ADCSDK.utils.generateId(res.name),
          res,
        ]) ?? [],
        remote?.plugin_configs?.map((res) => [
          res.name,
          ADCSDK.utils.generateId(res.name),
          res,
        ]) ?? [],
      ), */
    ];

    const unwrapedEvents: Array<ADCSDK.Event> = [];

    // unwrap sub events of a main event
    result.forEach((event) => {
      if (event.type !== ADCSDK.EventType.ONLY_SUB_EVENTS)
        unwrapedEvents.push(event);
      unwrapedEvents.push(...event.subEvents);
      unset(event, 'subEvents');
    });

    // sort by resource base rules
    const events = unwrapedEvents.sort((a, b) => {
      return (
        order[`${a.resourceType}.${a.type}`] -
        order[`${b.resourceType}.${b.type}`]
      );
    });

    differ.logger?.debug(
      { message: 'Diff result', transactionId: differ.transactionId, events },
      { showLogEntry },
    );

    return events;
  }

  private diffResource(
    resourceType: ADCSDK.ResourceType.SERVICE,
    local: Array<[ADCSDK.ResourceName, ADCSDK.ResourceId, ADCSDK.Service]>,
    remote: Array<[ADCSDK.ResourceName, ADCSDK.ResourceId, ADCSDK.Service]>,
  ): Array<ADCSDK.Event>;
  private diffResource(
    resourceType: ADCSDK.ResourceType.SSL,
    local: Array<[ADCSDK.ResourceName, ADCSDK.ResourceId, ADCSDK.SSL]>,
    remote: Array<[ADCSDK.ResourceName, ADCSDK.ResourceId, ADCSDK.SSL]>,
  ): Array<ADCSDK.Event>;
  private diffResource(
    resourceType: ADCSDK.ResourceType.CONSUMER,
    local: Array<[ADCSDK.ResourceName, ADCSDK.ResourceId, ADCSDK.Consumer]>,
    remote: Array<[ADCSDK.ResourceName, ADCSDK.ResourceId, ADCSDK.Consumer]>,
  ): Array<ADCSDK.Event>;
  private diffResource(
    resourceType: ADCSDK.ResourceType.GLOBAL_RULE,
    local: Array<[ADCSDK.ResourceName, ADCSDK.ResourceId, ADCSDK.GlobalRule]>,
    remote: Array<[ADCSDK.ResourceName, ADCSDK.ResourceId, ADCSDK.GlobalRule]>,
  ): Array<ADCSDK.Event>;
  private diffResource(
    resourceType: ADCSDK.ResourceType.PLUGIN_METADATA,
    local: Array<
      [ADCSDK.ResourceName, ADCSDK.ResourceId, ADCSDK.PluginMetadata]
    >,
    remote: Array<
      [ADCSDK.ResourceName, ADCSDK.ResourceId, ADCSDK.PluginMetadata]
    >,
  ): Array<ADCSDK.Event>;
  private diffResource(
    resourceType: ADCSDK.ResourceType.ROUTE,
    local: Array<[ADCSDK.ResourceName, ADCSDK.ResourceId, ADCSDK.Route]>,
    remote: Array<[ADCSDK.ResourceName, ADCSDK.ResourceId, ADCSDK.Route]>,
  ): Array<ADCSDK.Event>;
  private diffResource(
    resourceType: ADCSDK.ResourceType.STREAM_ROUTE,
    local: Array<[ADCSDK.ResourceName, ADCSDK.ResourceId, ADCSDK.StreamRoute]>,
    remote: Array<[ADCSDK.ResourceName, ADCSDK.ResourceId, ADCSDK.StreamRoute]>,
  ): Array<ADCSDK.Event>;
  private diffResource(
    resourceType: ADCSDK.ResourceType.CONSUMER_CREDENTIAL,
    local: Array<
      [ADCSDK.ResourceName, ADCSDK.ResourceId, ADCSDK.ConsumerCredential]
    >,
    remote: Array<
      [ADCSDK.ResourceName, ADCSDK.ResourceId, ADCSDK.ConsumerCredential]
    >,
  ): Array<ADCSDK.Event>;
  private diffResource(
    resourceType: ADCSDK.ResourceType.UPSTREAM,
    local: Array<[ADCSDK.ResourceName, ADCSDK.ResourceId, ADCSDK.Upstream]>,
    remote: Array<[ADCSDK.ResourceName, ADCSDK.ResourceId, ADCSDK.Upstream]>,
  ): Array<ADCSDK.Event>;
  private diffResource(
    resourceType: ADCSDK.ResourceType.CONSUMER_GROUP,
    local: Array<
      [ADCSDK.ResourceName, ADCSDK.ResourceId, ADCSDK.ConsumerGroup]
    >,
    remote: Array<
      [ADCSDK.ResourceName, ADCSDK.ResourceId, ADCSDK.ConsumerGroup]
    >,
  ): Array<ADCSDK.Event>;
  private diffResource(
    resourceType: ADCSDK.ResourceType.PLUGIN_CONFIG,
    local: Array<[ADCSDK.ResourceName, ADCSDK.ResourceId, ADCSDK.PluginConfig]>,
    remote: Array<
      [ADCSDK.ResourceName, ADCSDK.ResourceId, ADCSDK.PluginConfig]
    >,
  ): Array<ADCSDK.Event>;
  private diffResource<T extends ADCSDK.Resource>(
    resourceType: ADCSDK.ResourceType,
    local: Array<[ADCSDK.ResourceName, ADCSDK.ResourceId, T]> = [],
    remote: Array<[ADCSDK.ResourceName, ADCSDK.ResourceId, T]> = [],
  ): Array<ADCSDK.Event> {
    const result: Array<ADCSDK.Event> = [];

    const localIdMap: Record<ADCSDK.ResourceId, T> = Object.fromEntries(
      local.map((item) => [item[1], item[2]]),
    );

    const checkedRemoteId: Array<ADCSDK.ResourceId> = [];
    remote.forEach(([remoteName, remoteId, remoteItem]) => {
      remoteItem = cloneDeep(remoteItem); //TODO handle potentially immutable objects systematically
      unset(remoteItem, 'metadata');
      unset(remoteItem, 'id');

      // Asserts that the remote resource should exist locally, and that
      // non-existence means that the user deleted that resource.
      const localItem = localIdMap[remoteId];
      unset(localItem, 'id');

      // Exists remotely but not locally: resource deleted by user
      if (!localItem) {
        return result.push({
          resourceType,
          type: ADCSDK.EventType.DELETE,
          resourceId: remoteId,
          resourceName: remoteName,
          oldValue: remoteItem,

          // Special handling of resources containing nested resources
          // When a consumer is deleted, its credentials are not taken into
          // consideration.
          subEvents: DifferV3.diff(
            {},
            resourceType === ADCSDK.ResourceType.SERVICE
              ? {
                  routes: (remoteItem as ADCSDK.Service).routes,
                  stream_routes: (remoteItem as ADCSDK.Service).stream_routes,
                  upstreams: (remoteItem as ADCSDK.Service).upstreams,
                }
              : resourceType === ADCSDK.ResourceType.CONSUMER_GROUP
                ? {
                    consumers: (remoteItem as ADCSDK.ConsumerGroup).consumers,
                  }
                : resourceType ===
                    (ADCSDK.ResourceType.CONSUMER as ADCSDK.ResourceType)
                  ? {
                      consumer_credentials: (remoteItem as ADCSDK.Consumer)
                        .credentials,
                    }
                  : {},
            this.defaultValue,
            resourceType === ADCSDK.ResourceType.SERVICE
              ? remoteName
              : undefined,
            this.logger,
          ).map(this.postprocessSubEvent(remoteName, remoteId)),
        });
      }

      // Record the remote IDs that have been checked. It will be used
      // to identify locally added resources.
      checkedRemoteId.push(remoteId);

      const originalLocalItem = cloneDeep(localItem);

      // For special handling of SSL resources, since neither APISIX nor
      // API7 outputs certificate private keys in plaintext, the local
      // and remote key fields should be removed, and the check item should
      // be a combination of "snis + certificates", not including the key.
      // It is almost impossible to have the same public key value when the
      // private key is different, so the combination of SNI and certificate
      // public key checks the combination to find out the update of the
      // certificate in the local configuration.
      if (resourceType === ADCSDK.ResourceType.SSL) {
        (localItem as ADCSDK.SSL).certificates.forEach((cert) =>
          unset(cert, 'key'),
        );
        (remoteItem as ADCSDK.SSL).certificates.forEach((cert) =>
          unset(cert, 'key'),
        );
      }

      // Services will have two different default value rules depending on
      // the type of route they contain, one for HTTP services and another
      // for stream services.
      // Therefore, before merging the default values, we should decide
      // the default value table according to the type of service. This type
      // is only used for merging default values, other processes still
      // use the ResourceType.SERVICE type.
      const resourceTypeForDefault =
        resourceType != ADCSDK.ResourceType.SERVICE
          ? resourceType
          : has(localItem, 'stream_routes')
            ? ADCSDK.ResourceType.INTERNAL_STREAM_SERVICE
            : ADCSDK.ResourceType.SERVICE;

      // Merges local resources into a table of default values for this type.
      // The Admin API merges the default values specified in the schema into
      // the data, so default values not contained in the local data will
      // necessarily make it considered a data modification, and these
      // discrepancies will trigger update API calls, potentially resulting
      // in unintended behavior.
      // As such, each backend implementation needs to provide its table of
      // default values, which merges the local resources to the defaults,
      // just as the Admin API does.
      const defaultValue =
        this.defaultValue?.core?.[resourceTypeForDefault] ?? {};
      const mergedLocalItem = this.mergeDefault(
        localItem,
        cloneDeep(defaultValue),
      );

      // Special handling of resources containing nested resources: routes, consumer_groups
      const subEvents: Array<ADCSDK.Event> = [];
      if (
        [
          ADCSDK.ResourceType.SERVICE,
          ADCSDK.ResourceType.CONSUMER_GROUP,
          ADCSDK.ResourceType.CONSUMER,
        ].includes(resourceType)
      ) {
        subEvents.push(
          ...DifferV3.diff(
            resourceType ===
              (ADCSDK.ResourceType.SERVICE as ADCSDK.ResourceType)
              ? {
                  routes: (localItem as ADCSDK.Service).routes,
                  stream_routes: (localItem as ADCSDK.Service).stream_routes,
                  upstreams: (localItem as ADCSDK.Service).upstreams,
                }
              : resourceType ===
                  (ADCSDK.ResourceType.CONSUMER_GROUP as ADCSDK.ResourceType)
                ? {
                    consumers: (localItem as ADCSDK.ConsumerGroup).consumers,
                  }
                : resourceType ===
                    (ADCSDK.ResourceType.CONSUMER as ADCSDK.ResourceType)
                  ? {
                      consumer_credentials: (localItem as ADCSDK.Consumer)
                        .credentials,
                    }
                  : {},
            resourceType ===
              (ADCSDK.ResourceType.SERVICE as ADCSDK.ResourceType)
              ? {
                  routes: (remoteItem as ADCSDK.Service).routes,
                  stream_routes: (remoteItem as ADCSDK.Service).stream_routes,
                  upstreams: (remoteItem as ADCSDK.Service).upstreams,
                }
              : resourceType ===
                  (ADCSDK.ResourceType.CONSUMER_GROUP as ADCSDK.ResourceType)
                ? {
                    consumers: (remoteItem as ADCSDK.ConsumerGroup).consumers,
                  }
                : resourceType ===
                    (ADCSDK.ResourceType.CONSUMER as ADCSDK.ResourceType)
                  ? {
                      consumer_credentials: (remoteItem as ADCSDK.Consumer)
                        .credentials,
                    }
                  : {},
            this.defaultValue,
            [
              ADCSDK.ResourceType.SERVICE,
              ADCSDK.ResourceType.CONSUMER,
            ].includes(resourceType)
              ? remoteName
              : undefined,
            this.logger,
          ).map(this.postprocessSubEvent(remoteName, remoteId)),
        );

        this.logger?.debug(
          {
            message: 'Diff sub-resources',
            transactionId: this.transactionId,
            subEvents,
          },
          { showLogEntry },
        );

        // Remove nested resources to indeed compare the main resource itself.
        (resourceType === ADCSDK.ResourceType.SERVICE
          ? ['routes', 'stream_routes', 'upstreams']
          : resourceType === ADCSDK.ResourceType.CONSUMER_GROUP
            ? ['consumers']
            : ['credentials']
        ).map((key) => {
          unset(mergedLocalItem, key);
          unset(remoteItem, key);
        });
      }

      let outputLocalItem: ADCSDK.Resource = cloneDeep(originalLocalItem);
      let outputRemoteItem: ADCSDK.Resource = cloneDeep(remoteItem);

      // If the resource may contain plugin configurations, perform a
      // diff check on each plugin
      let pluginChanged = false;
      if (
        resourceType === ADCSDK.ResourceType.SERVICE ||
        resourceType === ADCSDK.ResourceType.CONSUMER ||
        resourceType === ADCSDK.ResourceType.ROUTE ||
        resourceType === ADCSDK.ResourceType.STREAM_ROUTE ||
        resourceType === ADCSDK.ResourceType.CONSUMER_GROUP ||
        resourceType === ADCSDK.ResourceType.PLUGIN_CONFIG
      ) {
        let mergedLocalPlugins: ADCSDK.Plugins = {};
        [pluginChanged, mergedLocalPlugins] = this.diffPlugins(
          'plugins' in mergedLocalItem
            ? (cloneDeep(mergedLocalItem.plugins) as ADCSDK.Plugins)
            : {},
          'plugins' in remoteItem ? (remoteItem.plugins as ADCSDK.Plugins) : {},
        );

        if (!isEmpty(mergedLocalPlugins))
          //@ts-expect-error it has been asserted that the resource type can contain the plugins field
          mergedLocalItem.plugins = mergedLocalPlugins;

        // The cache is used to output the old and new values of the diff report.
        // They are characterized by the fact that they do not contain subresources
        // such as route; however, they should be the current resource's containing
        // the plugins field.
        outputLocalItem = cloneDeep(mergedLocalItem);
        outputRemoteItem = cloneDeep(remoteItem);

        // Since plugins will be checked separately, differences in the plugins
        // should not be considered when checking for current resource changes.
        if (!pluginChanged) {
          unset(mergedLocalItem, 'plugins');
          unset(remoteItem, 'plugins');
        }
      }

      // Checking other resource properties, lhs is old(remote), rhs is new(local)
      // The resources checked are exclusively the current resource itself, without
      // plugins and sub-resources.
      const diff = objectDiff(cloneDeep(remoteItem), mergedLocalItem);
      this.logger?.debug(
        {
          message: 'Diff main resources',
          transactionId: this.transactionId,
          diff,
          local: outputLocalItem,
          remote: outputRemoteItem,
          realRemote: remoteItem,
          realLocal: mergedLocalItem,
          originalLocal: localItem,
        },
        { showLogEntry },
      );

      // If there are changes to the plugins or changes to other properties
      // of the resource, an update event is added.
      if (
        pluginChanged ||
        (diff && diff.length !== 0) ||
        subEvents.length > 0
      ) {
        // If there are only sub-events and no modifications to the resource
        // itself, only the ONLY_SUB_EVENTS event is emitted, and it will be
        // discarded during event aggregation and sorting.
        const onlySubEvents =
          subEvents.length > 0 &&
          !pluginChanged &&
          (!diff || diff.length === 0);

        return result.push({
          resourceType,
          type: onlySubEvents
            ? ADCSDK.EventType.ONLY_SUB_EVENTS
            : ADCSDK.EventType.UPDATE,
          resourceId: remoteId,
          resourceName: remoteName,
          oldValue: outputRemoteItem,

          // Merged defaults should not be included to prevent bothering the user
          newValue: outputLocalItem,

          // Even if only the plugin part has changed, the difference is recorded
          // on the whole resource.
          diff,

          // Attach sub resources update events
          subEvents,
        });
      }
    });

    // Exists locally but not remotely: resource created by user
    local.forEach(([localName, localId, localItem]) => {
      if (checkedRemoteId.includes(localId)) return;

      unset(localItem, 'metadata');
      unset(localItem, 'id');

      return result.push({
        resourceType,
        type: ADCSDK.EventType.CREATE,
        resourceId: localId,
        resourceName: localName,
        newValue: localItem,

        // Special handling of resources containing nested resources
        subEvents: DifferV3.diff(
          resourceType === ADCSDK.ResourceType.SERVICE
            ? {
                routes: (localItem as ADCSDK.Service).routes,
                stream_routes: (localItem as ADCSDK.Service).stream_routes,
                upstreams: (localItem as ADCSDK.Service).upstreams,
              }
            : resourceType === ADCSDK.ResourceType.CONSUMER_GROUP
              ? {
                  consumers: (localItem as ADCSDK.ConsumerGroup).consumers,
                }
              : resourceType === ADCSDK.ResourceType.CONSUMER
                ? {
                    consumer_credentials: (localItem as ADCSDK.Consumer)
                      .credentials,
                  }
                : {},
          {},
          this.defaultValue,
          [ADCSDK.ResourceType.SERVICE, ADCSDK.ResourceType.CONSUMER].includes(
            resourceType,
          )
            ? localName
            : undefined,
          this.logger,
        ).map(this.postprocessSubEvent(localName, localId)),
      });
    });

    return result;
  }

  // Check for differences on multiple plugins and return if any differences.
  private diffPlugins(
    local: ADCSDK.Plugins,
    remote: ADCSDK.Plugins,
  ): [boolean, ADCSDK.Plugins] {
    this.logger?.debug(
      {
        message: 'Diff plugins',
        transactionId: this.transactionId,
        local,
        remote,
        isEmptyLocal: isEmpty(local),
        isEmptyRemote: isEmpty(remote),
      },
      { showLogEntry },
    );
    if (isEmpty(local) && isEmpty(remote)) return [false, local];

    // Pre-merge the plugin's default configuration into the local configuration
    local = Object.fromEntries(
      Object.entries(local).map(([pluginName, config]) => {
        const defaultValue = this.defaultValue?.plugins?.[pluginName] ?? {};
        return [pluginName, this.mergeDefault(config, cloneDeep(defaultValue))];
      }),
    ) as ADCSDK.Plugins;
    const checker = (left: ADCSDK.Plugins, right: ADCSDK.Plugins) => {
      for (const leftPluginName in left) {
        const leftPlugin = left[leftPluginName];
        const rightPlugin = right[leftPluginName];

        // A plugin may be deleted, its configuration should be undefined.
        // When either plugin is configured as undefined, check if the two sides
        // are equal, and if they are not, it means that the plugin has been
        // added or removed.
        if (
          (leftPlugin === undefined || rightPlugin === undefined) &&
          !isEqual(leftPlugin, rightPlugin)
        )
          return true;

        const pluginDiff = objectDiff(
          cloneDeep(leftPlugin),
          cloneDeep(rightPlugin),
        );
        if (pluginDiff && pluginDiff.length !== 0) return true;
      }
      return false;
    };

    // Check for changes in the local plugin vs. remote and
    // remote plugin vs. local, respectively.
    return [checker(local, remote) || checker(remote, local), local];
  }

  private postprocessSubEvent(
    parentName: string,
    parentId: string,
  ): (event: ADCSDK.Event) => ADCSDK.Event {
    return (event) => {
      // If the Differ-generated subevent resource ID does not
      // match the resource name hash, the subevent resource ID
      // is maintained.
      // This may be due to the fact that the remote resource's
      // ID is server-generated.
      const subEventResourceId =
        ADCSDK.utils.generateId(event.resourceName) === event.resourceId
          ? ADCSDK.utils.generateId(`${parentName}.${event.resourceName}`)
          : event.resourceId;

      return {
        ...event,
        parentId,
        resourceId: subEventResourceId,
      };
    };
  }

  private mergeDefault(resource: ADCSDK.Resource, defaults: object): object {
    const defaultsClone = cloneDeep(defaults);
    const resourceClone = cloneDeep(resource);
    const isObjectButNotArray = (val: unknown) =>
      typeof val === 'object' && !Array.isArray(val);

    Object.entries(defaultsClone).forEach(([key, value]) => {
      // If a specific key does not exist in the resource
      if (isNil(resourceClone[key])) {
        // If the default value to be merged is an object and not an array,
        // it should not be merged.
        if (isObjectButNotArray(value) || Array.isArray(value)) return; // include any array value
        resourceClone[key] = value;
      } else {
        // If the default value and the value in the resource are both
        // objects, they need to be deeply merged, recursively it
        if (
          isObjectButNotArray(value) &&
          isObjectButNotArray(resourceClone[key])
        )
          resourceClone[key] = this.mergeDefault(resourceClone[key], value);

        // If the default value is an array then the default value of the
        // array item (index 0) is merged to each element of the array.
        if (
          Array.isArray(value) &&
          value[0] &&
          Array.isArray(resourceClone[key])
        )
          resourceClone[key] = (resourceClone[key] as Array<object>).map(
            (item) => this.mergeDefault(item, value[0]),
          );

        // Otherwise, ignore the default value and do not replace the
        // value in the resource
      }
    });

    return resourceClone;
  }
}
