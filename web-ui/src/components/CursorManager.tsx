import { useEffect } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { useAssets } from '../contexts';

interface CursorChangeEvent {
  cursor_type: 'default' | 'add' | 'attack' | 'impossible' | 'talk';
}

const CURSOR_HOTSPOTS = {
  default: { x: 17, y: 17 },
  add: { x: 17, y: 17 },
  attack: { x: 10, y: 5 },
  impossible: { x: 17, y: 17 },
  talk: { x: 17, y: 17 },
};

/**
 * CursorManager - Manages custom game cursor based on game state
 *
 * Listens to cursor-change events from Bevy via Tauri IPC and updates
 * the document cursor using CSS custom cursors with proper hotspot positioning.
 */
export function CursorManager() {
  const {
    cursorDefaultUrl,
    cursorAddUrl,
    cursorAttackUrl,
    cursorImpossibleUrl,
    cursorTalkUrl,
  } = useAssets();

  useEffect(() => {
    if (!cursorDefaultUrl || !cursorAddUrl || !cursorAttackUrl ||
        !cursorImpossibleUrl || !cursorTalkUrl) {
      return;
    }

    const cursorUrls = {
      default: cursorDefaultUrl,
      add: cursorAddUrl,
      attack: cursorAttackUrl,
      impossible: cursorImpossibleUrl,
      talk: cursorTalkUrl,
    };

    const updateCursor = (cursorType: CursorChangeEvent['cursor_type']) => {
      const url = cursorUrls[cursorType];
      const hotspot = CURSOR_HOTSPOTS[cursorType];

      if (!url) {
        console.error(`[CursorManager] Missing cursor URL for type: ${cursorType}`);
        return;
      }

      document.body.style.cursor = `url(${url}) ${hotspot.x} ${hotspot.y}, auto`;
    };

    let unlistenFn: UnlistenFn | null = null;

    const setupListener = async () => {
      try {
        unlistenFn = await listen<CursorChangeEvent>('cursor-change', (event) => {
          const { cursor_type } = event.payload;
          updateCursor(cursor_type);
        });

        updateCursor('default');
      } catch (error) {
        console.error('[CursorManager] Failed to set up cursor-change listener:', error);
      }
    };

    setupListener();

    return () => {
      if (unlistenFn) {
        unlistenFn();
      }
      document.body.style.cursor = '';
    };
  }, [cursorDefaultUrl, cursorAddUrl, cursorAttackUrl, cursorImpossibleUrl, cursorTalkUrl]);

  return null;
}
