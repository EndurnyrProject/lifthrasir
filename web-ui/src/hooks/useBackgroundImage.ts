import { useState, useEffect } from 'react';
import { loadAsset } from '../lib/assets';

interface UseBackgroundImageResult {
  backgroundUrl: string | null;
  isLoading: boolean;
  error: string | null;
}

/**
 * Hook for loading background images with loading state
 *
 * Ensures the background is loaded before rendering the UI,
 * preventing the "pop-in" effect where UI appears before the background.
 *
 * @param assetPath - Path to the background image asset
 * @returns Object with backgroundUrl, isLoading, and error state
 *
 * @example
 * ```tsx
 * const { backgroundUrl, isLoading, error } = useBackgroundImage('login_screen.png');
 *
 * if (isLoading) {
 *   return <LoadingScreen />;
 * }
 *
 * return <div style={{ backgroundImage: `url(${backgroundUrl})` }}>...</div>;
 * ```
 */
export function useBackgroundImage(assetPath: string): UseBackgroundImageResult {
  const [backgroundUrl, setBackgroundUrl] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let isMounted = true;

    const loadBackground = async () => {
      try {
        setIsLoading(true);
        const url = await loadAsset(assetPath);

        if (isMounted) {
          setBackgroundUrl(url);
          setError(null);
        }
      } catch (err) {
        if (isMounted) {
          setError(`Failed to load background: ${err}`);
        }
      } finally {
        if (isMounted) {
          setIsLoading(false);
        }
      }
    };

    loadBackground();

    return () => {
      isMounted = false;
      if (backgroundUrl) {
        URL.revokeObjectURL(backgroundUrl);
      }
    };
  }, [assetPath]);

  return { backgroundUrl, isLoading, error };
}
