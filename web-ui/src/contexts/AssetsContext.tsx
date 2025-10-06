import { createContext, useContext, useState, useEffect, ReactNode } from 'react';
import { loadAsset } from '../lib/assets';

interface AssetsContextValue {
  backgroundUrl: string | null;
  slotWithCharUrl: string | null;
  slotNoCharUrl: string | null;
  slotBlockedUrl: string | null;
  isLoading: boolean;
  error: string | null;
}

const AssetsContext = createContext<AssetsContextValue | undefined>(undefined);

interface AssetsProviderProps {
  children: ReactNode;
}

/**
 * AssetsProvider - Preloads all application assets at startup
 *
 * This provider loads all shared assets (backgrounds, etc.) once at app initialization,
 * eliminating loading screens during screen transitions and preventing gray backgrounds.
 *
 * All child components can access preloaded assets via useAssets() hook.
 */
export function AssetsProvider({ children }: AssetsProviderProps) {
  const [backgroundUrl, setBackgroundUrl] = useState<string | null>(null);
  const [slotWithCharUrl, setSlotWithCharUrl] = useState<string | null>(null);
  const [slotNoCharUrl, setSlotNoCharUrl] = useState<string | null>(null);
  const [slotBlockedUrl, setSlotBlockedUrl] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const preloadAssets = async () => {
      try {
        setIsLoading(true);

        // Preload the login screen background (used by all screens)
        const bgUrl = await loadAsset('login_screen.png');
        setBackgroundUrl(bgUrl);

        // Preload character slot background images
        const slotWithChar = await loadAsset('textures/ui/character_screen/slot_with_char.png');
        setSlotWithCharUrl(slotWithChar);

        const slotNoChar = await loadAsset('textures/ui/character_screen/slot_no_char.png');
        setSlotNoCharUrl(slotNoChar);

        const slotBlocked = await loadAsset('textures/ui/character_screen/slot_blocked_char.png');
        setSlotBlockedUrl(slotBlocked);

        setError(null);
      } catch (err) {
        setError(`Failed to load assets: ${err}`);
        console.error('Asset preloading failed:', err);
      } finally {
        setIsLoading(false);
      }
    };

    preloadAssets();

    // Cleanup on unmount
    return () => {
      if (backgroundUrl) URL.revokeObjectURL(backgroundUrl);
      if (slotWithCharUrl) URL.revokeObjectURL(slotWithCharUrl);
      if (slotNoCharUrl) URL.revokeObjectURL(slotNoCharUrl);
      if (slotBlockedUrl) URL.revokeObjectURL(slotBlockedUrl);
    };
  }, []);

  return (
    <AssetsContext.Provider value={{ backgroundUrl, slotWithCharUrl, slotNoCharUrl, slotBlockedUrl, isLoading, error }}>
      {children}
    </AssetsContext.Provider>
  );
}

/**
 * useAssets - Hook to access preloaded assets
 *
 * @returns Object containing backgroundUrl, isLoading, and error state
 * @throws Error if used outside AssetsProvider
 *
 * @example
 * ```tsx
 * function MyScreen() {
 *   const { backgroundUrl } = useAssets();
 *   return <div style={{ backgroundImage: `url(${backgroundUrl})` }}>...</div>;
 * }
 * ```
 */
export function useAssets(): AssetsContextValue {
  const context = useContext(AssetsContext);
  if (context === undefined) {
    throw new Error('useAssets must be used within an AssetsProvider');
  }
  return context;
}
