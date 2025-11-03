import { useState, useEffect } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import './EntityTooltip.css';

interface EntityTooltipData {
  entity_id: number;
  name: string;
  party_name?: string;
  guild_name?: string;
  position_name?: string;
  screen_x: number;
  screen_y: number;
}

export function EntityTooltip() {
  const [tooltipData, setTooltipData] = useState<EntityTooltipData | null>(null);

  useEffect(() => {
    const unlistenPromises: Promise<UnlistenFn>[] = [];

    unlistenPromises.push(
      listen<EntityTooltipData>('entity-name-show', (event) => {
        setTooltipData(event.payload);
      })
    );

    unlistenPromises.push(
      listen('entity-name-hide', () => {
        setTooltipData(null);
      })
    );

    return () => {
      unlistenPromises.forEach(promise => {
        promise.then(unlisten => unlisten()).catch(console.error);
      });
    };
  }, []);

  if (!tooltipData) {
    return null;
  }

  return (
    <div
      className="entity-tooltip"
      style={{
        left: `${tooltipData.screen_x}px`,
        top: `${tooltipData.screen_y - 40}px`,
      }}
    >
      <div className="entity-tooltip-name">{tooltipData.name}</div>
      {tooltipData.party_name && (
        <div className="entity-tooltip-party">Party: {tooltipData.party_name}</div>
      )}
      {tooltipData.guild_name && (
        <div className="entity-tooltip-guild">Guild: {tooltipData.guild_name}</div>
      )}
      {tooltipData.position_name && (
        <div className="entity-tooltip-position">{tooltipData.position_name}</div>
      )}
    </div>
  );
}
