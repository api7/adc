import * as ADCSDK from '@api7/adc-sdk';

import { DifferV3 } from '../differv3.js';

describe('Differ V3 - custom plugin', () => {
  const name = 'my-plugin';
  const content = 'local _M = { name = "my-plugin" }\nreturn _M';

  it('should create custom plugin', () => {
    expect(DifferV3.diff({ custom_plugins: [{ name, content }] }, {})).toEqual([
      {
        resourceType: ADCSDK.ResourceType.CUSTOM_PLUGIN,
        type: ADCSDK.EventType.CREATE,
        resourceId: name,
        resourceName: name,
        newValue: { name, content },
      },
    ] as Array<ADCSDK.Event>);
  });

  it('should update custom plugin when the source changes', () => {
    const newContent = `${content}\n-- updated`;
    const events = DifferV3.diff(
      { custom_plugins: [{ name, content: newContent }] },
      { custom_plugins: [{ name, content }] },
    );
    expect(events).toHaveLength(1);
    expect(events[0]).toMatchObject({
      resourceType: ADCSDK.ResourceType.CUSTOM_PLUGIN,
      type: ADCSDK.EventType.UPDATE,
      resourceId: name,
      resourceName: name,
      newValue: { name, content: newContent },
    });
  });

  it('should not emit events when the custom plugin is unchanged', () => {
    expect(
      DifferV3.diff(
        { custom_plugins: [{ name, content }] },
        { custom_plugins: [{ name, content }] },
      ),
    ).toEqual([]);
  });

  it('should delete (prune) a custom plugin missing locally', () => {
    const events = DifferV3.diff({}, { custom_plugins: [{ name, content }] });
    expect(events).toHaveLength(1);
    expect(events[0]).toMatchObject({
      resourceType: ADCSDK.ResourceType.CUSTOM_PLUGIN,
      type: ADCSDK.EventType.DELETE,
      resourceId: name,
      resourceName: name,
      oldValue: { name, content },
    });
  });

  it('should order custom plugin creation before route creation', () => {
    const events = DifferV3.diff(
      {
        custom_plugins: [{ name, content }],
        routes: [{ name: 'r1', uris: ['/'], plugins: { [name]: {} } }],
      },
      {},
    );
    const types = events.map((e) => e.resourceType);
    expect(types.indexOf(ADCSDK.ResourceType.CUSTOM_PLUGIN)).toBeLessThan(
      types.indexOf(ADCSDK.ResourceType.ROUTE),
    );
  });
});
