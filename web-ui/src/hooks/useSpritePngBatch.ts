import { useState, useEffect, useCallback } from 'react';
import { getSpritePng, SpritePngRequest, SpritePngResponse } from '../lib/spritePng';

/**
 * Result from useSpritePngBatch hook
 */
export interface UseSpritePngBatchResult {
    /** Map of request key to sprite response */
    sprites: Map<string, SpritePngResponse>;
    /** Whether any sprites are currently loading */
    loading: boolean;
    /** Error message if batch loading failed, or null */
    error: string | null;
    /** Progress information: { loaded: number, total: number } */
    progress: { loaded: number; total: number };
    /** Function to manually refetch all sprites */
    refetch: () => void;
}

/**
 * Generate a unique key for a sprite request
 */
function getRequestKey(request: SpritePngRequest): string {
    return JSON.stringify({
        sprite_path: request.sprite_path,
        act_path: request.act_path,
        action_index: request.action_index,
        frame_index: request.frame_index,
        palette_path: request.palette_path,
        scale: request.scale ?? 1.0,
    });
}

/**
 * Custom hook for batch loading multiple sprite PNGs
 *
 * Loads all sprites in parallel with progress tracking.
 * Continues loading even if individual sprites fail.
 *
 * @param requests Array of sprite requests to load (empty array loads nothing)
 * @returns Sprites map, loading state, error, progress, and refetch function
 *
 * @example
 * ```typescript
 * function CharacterCustomizer() {
 *     const hairStyles = [
 *         { sprite_path: 'hair1.spr', action_index: 0, frame_index: 0 },
 *         { sprite_path: 'hair2.spr', action_index: 0, frame_index: 0 },
 *         { sprite_path: 'hair3.spr', action_index: 0, frame_index: 0 },
 *     ];
 *
 *     const { sprites, loading, progress } = useSpritePngBatch(hairStyles);
 *
 *     if (loading) {
 *         return <div>Loading {progress.loaded}/{progress.total} sprites...</div>;
 *     }
 *
 *     return (
 *         <div>
 *             {hairStyles.map((req) => {
 *                 const key = JSON.stringify(req);
 *                 const sprite = sprites.get(key);
 *                 return sprite ? <img key={key} src={sprite.data_url} /> : null;
 *             })}
 *         </div>
 *     );
 * }
 * ```
 */
export function useSpritePngBatch(requests: SpritePngRequest[]): UseSpritePngBatchResult {
    const [sprites, setSprites] = useState<Map<string, SpritePngResponse>>(new Map());
    const [loading, setLoading] = useState<boolean>(false);
    const [error, setError] = useState<string | null>(null);
    const [progress, setProgress] = useState<{ loaded: number; total: number }>({
        loaded: 0,
        total: 0,
    });

    // Create stable requests key for dependency array
    const requestsKey = JSON.stringify(
        requests.map(req => ({
            sprite_path: req.sprite_path,
            act_path: req.act_path,
            action_index: req.action_index,
            frame_index: req.frame_index,
            palette_path: req.palette_path,
            scale: req.scale ?? 1.0,
        }))
    );

    const loadSprites = useCallback(async () => {
        if (requests.length === 0) {
            setSprites(new Map());
            setLoading(false);
            setError(null);
            setProgress({ loaded: 0, total: 0 });
            return;
        }

        setLoading(true);
        setError(null);
        setProgress({ loaded: 0, total: requests.length });

        const newSprites = new Map<string, SpritePngResponse>();
        let loadedCount = 0;
        let hasError = false;

        // Load all sprites in parallel
        const loadPromises = requests.map(async (request) => {
            try {
                const response = await getSpritePng(request);
                const key = getRequestKey(request);
                newSprites.set(key, response);
            } catch (err) {
                // Log error but continue with other sprites
                const errorMessage = err instanceof Error ? err.message : String(err);
                console.error(`Failed to load sprite: ${errorMessage}`, request);
                hasError = true;
            } finally {
                loadedCount++;
                setProgress({ loaded: loadedCount, total: requests.length });
            }
        });

        await Promise.all(loadPromises);

        setSprites(newSprites);
        setLoading(false);

        if (hasError && newSprites.size === 0) {
            // All sprites failed to load
            setError('Failed to load all sprites');
        } else if (hasError) {
            // Some sprites failed to load
            setError(`Failed to load ${requests.length - newSprites.size} of ${requests.length} sprites`);
        }
    }, [requestsKey]);

    useEffect(() => {
        loadSprites();
    }, [loadSprites]);

    const refetch = useCallback(() => {
        loadSprites();
    }, [loadSprites]);

    return {
        sprites,
        loading,
        error,
        progress,
        refetch,
    };
}
