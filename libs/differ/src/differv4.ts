import * as ADCSDK from '@api7/adc-sdk';
import { FieldListType } from '@api7/adc-sdk';
import { randomUUID } from 'crypto';
import { type Diff, diff as objectDiff } from 'datum-diff';
import { cloneDeep, isEmpty, isEqual, isNil, unset } from 'lodash-es';

/**
 * Event ordering table: deletions precede creates, SSL creates precede routes
 * (SSL may be referenced by upstream mTLS and must exist first).
 * Typed as Partial so ONLY_SUB_EVENTS and runtime-only types need no placeholder entries.
 */
const order: Partial<Record<`${ADCSDK.ResourceType}.${ADCSDK.EventType}`, number>> = {
  [`${ADCSDK.ResourceType.ROUTE}.${ADCSDK.EventType.DELETE}`]: 0,
  [`${ADCSDK.ResourceType.STREAM_ROUTE}.${ADCSDK.EventType.DELETE}`]: 1,
  [`${ADCSDK.ResourceType.SERVICE}.${ADCSDK.EventType.DELETE}`]: 2,
  [`${ADCSDK.ResourceType.UPSTREAM}.${ADCSDK.EventType.DELETE}`]: 3,
  [`${ADCSDK.ResourceType.PLUGIN_CONFIG}.${ADCSDK.EventType.DELETE}`]: 4,
  [`${ADCSDK.ResourceType.CONSUMER}.${ADCSDK.EventType.DELETE}`]: 5,
  [`${ADCSDK.ResourceType.CONSUMER_GROUP}.${ADCSDK.EventType.DELETE}`]: 6,
  [`${ADCSDK.ResourceType.SSL}.${ADCSDK.EventType.DELETE}`]: 7,

  [`${ADCSDK.ResourceType.SSL}.${ADCSDK.EventType.CREATE}`]: 8,
  [`${ADCSDK.ResourceType.SSL}.${ADCSDK.EventType.UPDATE}`]: 9,
  [`${ADCSDK.ResourceType.ROUTE}.${ADCSDK.EventType.UPDATE}`]: 10,
  [`${ADCSDK.ResourceType.STREAM_ROUTE}.${ADCSDK.EventType.UPDATE}`]: 11,
  [`${ADCSDK.ResourceType.SERVICE}.${ADCSDK.EventType.UPDATE}`]: 12,
  [`${ADCSDK.ResourceType.UPSTREAM}.${ADCSDK.EventType.UPDATE}`]: 13,
  [`${ADCSDK.ResourceType.PLUGIN_CONFIG}.${ADCSDK.EventType.UPDATE}`]: 14,
  [`${ADCSDK.ResourceType.CONSUMER_GROUP}.${ADCSDK.EventType.UPDATE}`]: 15,
  [`${ADCSDK.ResourceType.CONSUMER}.${ADCSDK.EventType.UPDATE}`]: 16,

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

type ResourceTuple = [ADCSDK.ResourceName, ADCSDK.ResourceId, unknown];

export class DifferV4 {
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
    local: ADCSDK.InternalConfiguration,
    remote: ADCSDK.InternalConfiguration,
    defaultValue?: ADCSDK.DefaultValue,
    parentName?: string,
    logger?: ADCSDK.Logger,
  ): Array<ADCSDK.Event> {
    const differ = new DifferV4({
      transactionId: randomUUID(),
      defaultValue: defaultValue ?? {},
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

    const result: Array<ADCSDK.Event> = [];
    for (const [type, meta] of Object.entries(ADCSDK.RESOURCE_DIFFER_META)) {
      const resourceType = type as ADCSDK.ResourceType;
      if (!meta.configField) continue;

      result.push(
        ...differ.diffResource(
          resourceType,
          meta,
          differ.extractTuples(local, meta, parentName),
          differ.extractTuples(remote, meta),
        ),
      );
    }

    const unwrappedEvents: Array<ADCSDK.Event> = [];
    result.forEach((event) => {
      if (event.type !== ADCSDK.EventType.ONLY_SUB_EVENTS) unwrappedEvents.push(event);
      if (event.subEvents) unwrappedEvents.push(...event.subEvents);
      unset(event, 'subEvents');
    });

    const events = unwrappedEvents.sort(
      (a, b) =>
        (order[`${a.resourceType}.${a.type}`] ?? Infinity) -
        (order[`${b.resourceType}.${b.type}`] ?? Infinity),
    );

    differ.logger?.debug(
      { message: 'Diff result', transactionId: differ.transactionId, events },
      { showLogEntry },
    );

    return events;
  }

  /** Build [name, id, item] tuples from an InternalConfiguration field. */
  private extractTuples(
    config: ADCSDK.InternalConfiguration,
    meta: ADCSDK.ResourceDifferMeta,
    parentName?: string,
  ): ResourceTuple[] {
    const { configField } = meta;
    if (!configField) return [];
    const field = config[configField];
    if (!field) return [];

    if (meta.collectionKind === ADCSDK.CollectionKind.RECORD) {
      return Object.entries(field as Record<string, unknown>).map(([key, value]) => [
        key,
        key,
        value,
      ]);
    }

    return (field as unknown[]).map((item) => {
      const name = meta.getName(item);
      // Always pass parentName to generateId; each type's implementation decides whether to use it.
      const id = meta.generateId(item, parentName);
      return [name, id, item];
    });
  }

  private diffResource(
    resourceType: ADCSDK.ResourceType,
    meta: ADCSDK.ResourceDifferMeta,
    local: ResourceTuple[],
    remote: ResourceTuple[],
  ): Array<ADCSDK.Event> {
    const result: Array<ADCSDK.Event> = [];
    const localIdMap = new Map<ADCSDK.ResourceId, unknown>(
      local.map(([, id, item]) => [id, item]),
    );
    const seenRemoteIds = new Set<ADCSDK.ResourceId>();

    remote.forEach(([remoteName, remoteId, rawRemoteItem]) => {
      const remoteItem = this.prepareRemoteItem(rawRemoteItem);
      const localItem = localIdMap.get(remoteId);
      unset(localItem, 'id');

      if (!localItem) {
        result.push(
          this.handleDelete(meta, resourceType, remoteId, remoteName, remoteItem),
        );
        return;
      }

      seenRemoteIds.add(remoteId);
      const event = this.handleUpdate(
        meta,
        resourceType,
        remoteId,
        remoteName,
        localItem,
        remoteItem,
      );
      if (event) result.push(event);
    });

    local.forEach(([localName, localId, localItem]) => {
      if (seenRemoteIds.has(localId)) return;
      unset(localItem, 'id');
      result.push(this.handleCreate(meta, resourceType, localId, localName, localItem));
    });

    return result;
  }

  private prepareRemoteItem(raw: unknown): Record<string, unknown> {
    const item = cloneDeep(raw) as Record<string, unknown>;
    unset(item, 'id');
    return item;
  }

  private handleDelete(
    meta: ADCSDK.ResourceDifferMeta,
    resourceType: ADCSDK.ResourceType,
    remoteId: string,
    remoteName: string,
    remoteItem: Record<string, unknown>,
  ): ADCSDK.Event {
    const subConfig = this.extractSubConfig(meta, remoteItem);
    return {
      resourceType,
      type: ADCSDK.EventType.DELETE,
      resourceId: remoteId,
      resourceName: remoteName,
      oldValue: remoteItem,
      subEvents: DifferV4.diff(
        {},
        subConfig,
        this.defaultValue,
        meta.propagatesParentName ? remoteName : undefined,
        this.logger,
      ).map(this.postprocessSubEvent(remoteName, remoteId)),
    };
  }

  private handleCreate(
    meta: ADCSDK.ResourceDifferMeta,
    resourceType: ADCSDK.ResourceType,
    localId: string,
    localName: string,
    localItem: unknown,
  ): ADCSDK.Event {
    const subConfig = this.extractSubConfig(meta, localItem);
    return {
      resourceType,
      type: ADCSDK.EventType.CREATE,
      resourceId: localId,
      resourceName: localName,
      newValue: localItem as ADCSDK.ResourceFor<ADCSDK.ResourceType>,
      subEvents: DifferV4.diff(
        subConfig,
        {},
        this.defaultValue,
        meta.propagatesParentName ? localName : undefined,
        this.logger,
      ).map(this.postprocessSubEvent(localName, localId)),
    };
  }

  private handleUpdate(
    meta: ADCSDK.ResourceDifferMeta,
    resourceType: ADCSDK.ResourceType,
    remoteId: string,
    remoteName: string,
    localItem: unknown,
    remoteItem: Record<string, unknown>,
  ): ADCSDK.Event | null {
    const originalLocalItem = cloneDeep(localItem);

    // Apply atomic field stripping declared in schema .meta() (e.g. SSL cert keys)
    this.applyAtomicStrips(meta, localItem, remoteItem);

    // Resolve the default-value type (SERVICE may be a stream service)
    const defaultType = meta.resolveDefaultType?.(localItem) ?? resourceType;
    const defaultValue = this.defaultValue?.core?.[defaultType] ?? {};
    const mergedLocalItem = this.mergeDefault(
      localItem as ADCSDK.ResourceFor<ADCSDK.ResourceType>,
      cloneDeep(defaultValue),
    );

    // Diff nested sub-resources (routes, credentials, consumers, upstreams within a service)
    const subEvents: Array<ADCSDK.Event> = [];
    const nestedFields = Object.entries(meta.fields).filter(
      ([, fm]) => fm.listType === FieldListType.MAP && fm.nested,
    );
    if (nestedFields.length > 0) {
      const localSubConfig = this.extractSubConfig(meta, localItem);
      const remoteSubConfig = this.extractSubConfig(meta, remoteItem);
      subEvents.push(
        ...DifferV4.diff(
          localSubConfig,
          remoteSubConfig,
          this.defaultValue,
          meta.propagatesParentName ? remoteName : undefined,
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

      // Strip nested keys from the items before comparing the parent resource body
      nestedFields.forEach(([key]) => {
        unset(mergedLocalItem, key);
        unset(remoteItem, key);
      });
    }

    // Diff plugins via objectMap fields (bidirectional comparison handles additions/deletions)
    // Snapshot remote before plugin stripping so oldValue in the event retains plugin fields
    const outputRemoteItem = cloneDeep(remoteItem);
    let pluginChanged = false;
    let outputLocalItem: unknown = originalLocalItem;

    const objectMapFields = Object.entries(meta.fields).filter(
      ([, fm]) => fm.listType === FieldListType.OBJECT_MAP,
    );
    if (objectMapFields.length > 0) {
      for (const [fieldName] of objectMapFields) {
        const localPlugins =
          fieldName in (mergedLocalItem as object)
            ? (cloneDeep((mergedLocalItem as Record<string, unknown>)[fieldName]) as ADCSDK.Plugins)
            : {};
        const remotePlugins =
          fieldName in remoteItem
            ? (remoteItem[fieldName] as ADCSDK.Plugins)
            : {};

        const [fieldChanged, mergedLocalPlugins] = this.diffPlugins(localPlugins, remotePlugins);
        if (fieldChanged) pluginChanged = true;

        if (!isEmpty(mergedLocalPlugins)) {
          (mergedLocalItem as Record<string, unknown>)[fieldName] = mergedLocalPlugins;
        }
      }

      // Capture output before stripping unchanged plugin fields from the comparison items
      outputLocalItem = cloneDeep(mergedLocalItem);
      if (!pluginChanged) {
        objectMapFields.forEach(([fieldName]) => {
          unset(mergedLocalItem, fieldName);
          unset(remoteItem, fieldName);
        });
      }
    }

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

    if (!pluginChanged && (!diff || diff.length === 0) && subEvents.length === 0) return null;

    const onlySubEvents =
      subEvents.length > 0 && !pluginChanged && (!diff || diff.length === 0);

    return {
      resourceType,
      type: onlySubEvents ? ADCSDK.EventType.ONLY_SUB_EVENTS : ADCSDK.EventType.UPDATE,
      resourceId: remoteId,
      resourceName: remoteName,
      oldValue: outputRemoteItem,
      newValue: outputLocalItem as ADCSDK.ResourceFor<ADCSDK.ResourceType>,
      diff: diff as Array<Diff<ADCSDK.ResourceFor<ADCSDK.ResourceType>, object>>,
      subEvents,
    };
  }

  /**
   * Build an InternalConfiguration containing only the nested sub-resource fields
   * declared as {listType:'map', nested:true} in the resource's schema metadata.
   */
  private extractSubConfig(
    meta: ADCSDK.ResourceDifferMeta,
    item: unknown,
  ): ADCSDK.InternalConfiguration {
    const subConfig: ADCSDK.InternalConfiguration = {};
    for (const [fieldName, fieldMeta] of Object.entries(meta.fields)) {
      if (fieldMeta.listType !== FieldListType.MAP || !fieldMeta.nested) continue;
      const value = (item as Record<string, unknown>)?.[fieldName];
      if (value !== undefined) {
        // Map the resource field name to the corresponding InternalConfiguration key.
        // 'credentials' on Consumer maps to 'consumer_credentials' in InternalConfiguration.
        const configKey =
          fieldName === 'credentials' ? 'consumer_credentials' : fieldName;
        (subConfig as Record<string, unknown>)[configKey] = value;
      }
    }
    return subConfig;
  }

  /**
   * Strip atomic sub-fields from array items as declared by {listType:'array', stripItemFields}.
   * Used for SSL certificates: the private key is removed before comparison because the remote
   * never returns it in plaintext.
   */
  private applyAtomicStrips(
    meta: ADCSDK.ResourceDifferMeta,
    localItem: unknown,
    remoteItem: Record<string, unknown>,
  ): void {
    for (const [fieldName, fieldMeta] of Object.entries(meta.fields)) {
      if (fieldMeta.listType !== FieldListType.ARRAY || !fieldMeta.stripItemFields?.length) continue;
      for (const subField of fieldMeta.stripItemFields) {
        ((localItem as Record<string, unknown>)[fieldName] as unknown[] | undefined)?.forEach(
          (item) => unset(item, subField),
        );
        (remoteItem[fieldName] as unknown[] | undefined)?.forEach((item) =>
          unset(item, subField),
        );
      }
    }
  }

  private postprocessSubEvent(
    parentName: string,
    parentId: string,
  ): (event: ADCSDK.Event) => ADCSDK.Event {
    return (event) => {
      const subEventResourceId =
        ADCSDK.utils.generateId(event.resourceName) === event.resourceId
          ? ADCSDK.utils.generateId(`${parentName}.${event.resourceName}`)
          : event.resourceId;
      return { ...event, parentId, resourceId: subEventResourceId };
    };
  }

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

    local = Object.fromEntries(
      Object.entries(local).map(([pluginName, config]) => {
        const defaultValue = this.defaultValue?.plugins?.[pluginName] ?? {};
        return [
          pluginName,
          this.mergeDefault(config as ADCSDK.Plugin, cloneDeep(defaultValue)),
        ];
      }),
    ) as ADCSDK.Plugins;

    const checker = (left: ADCSDK.Plugins, right: ADCSDK.Plugins) => {
      for (const leftPluginName in left) {
        const leftPlugin = left[leftPluginName];
        const rightPlugin = right[leftPluginName];

        if (
          (leftPlugin === undefined || rightPlugin === undefined) &&
          !isEqual(leftPlugin, rightPlugin)
        )
          return true;

        const pluginDiff = objectDiff(leftPlugin, rightPlugin);
        if (pluginDiff && pluginDiff.length !== 0) return true;
      }
      return false;
    };

    return [checker(local, remote) || checker(remote, local), local];
  }

  private mergeDefault<T extends ADCSDK.ResourceType>(
    resource: ADCSDK.ResourceFor<T>,
    defaults: object,
  ): object {
    const defaultsClone = cloneDeep(defaults);
    const resourceClone = cloneDeep(resource) as Record<string, unknown>;
    const isObjectButNotArray = (val: unknown) =>
      typeof val === 'object' && !Array.isArray(val);

    Object.entries(defaultsClone).forEach(([key, value]) => {
      if (key === '__proto__' || key === 'constructor' || key === 'prototype') return;
      if (isNil(resourceClone[key])) {
        if (isObjectButNotArray(value) || Array.isArray(value)) return;
        resourceClone[key] = value;
      } else {
        if (isObjectButNotArray(value) && isObjectButNotArray(resourceClone[key]))
          resourceClone[key] = this.mergeDefault(resourceClone[key], value);

        if (Array.isArray(value) && value[0] && Array.isArray(resourceClone[key]))
          resourceClone[key] = (resourceClone[key] as Array<object>).map((item) =>
            this.mergeDefault(item, value[0]),
          );
      }
    });

    return resourceClone;
  }
}
