import { useState, useEffect, useCallback } from 'react';
import { getSpritePng, SpritePngRequest, SpritePngResponse } from '../lib/spritePng';

/**
 * Options for useSpritePng hook
 */
export interface UseSpritePngOptions extends Omit<SpritePngRequest, 'action_index' | 'frame_index'> {
    /** Action index (default: 0) */
    action_index?: number;
    /** Frame index (default: 0) */
    frame_index?: number;
}

/**
 * Result from useSpritePng hook
 */
export interface UseSpritePngResult {
    /** Sprite PNG response with data URL, or null if not loaded */
    sprite: SpritePngResponse | null;
    /** Whether the sprite is currently loading */
    loading: boolean;
    /** Error message if loading failed, or null */
    error: string | null;
    /** Function to manually refetch the sprite */
    refetch: () => void;
}

/**
 * Custom hook for loading a single sprite PNG
 *
 * Automatically loads the sprite on mount and whenever parameters change.
 * Supports null request for conditional rendering.
 *
 * @param request Sprite request parameters (null to skip loading)
 * @returns Sprite data, loading state, error, and refetch function
 *
 * @example
 * ```typescript
 * function MyComponent() {
 *     const { sprite, loading, error } = useSpritePng({
 *         sprite_path: 'data\\sprite\\인간족\\몸통\\여\\여_body.spr',
 *         action_index: 0,
 *         frame_index: 0,
 *         scale: 2.0
 *     });
 *
 *     if (loading) return <div>Loading sprite...</div>;
 *     if (error) return <div>Error: {error}</div>;
 *     if (!sprite) return null;
 *
 *     return <img src={sprite.data_url} alt="Character sprite" />;
 * }
 * ```
 *
 * @example
 * ```typescript
 * // Conditional loading
 * const { sprite, loading } = useSpritePng(
 *     showSprite ? { sprite_path: 'hair.spr', action_index: 0, frame_index: 0 } : null
 * );
 * ```
 */
export function useSpritePng(request: UseSpritePngOptions | null): UseSpritePngResult {
    const [sprite, setSprite] = useState<SpritePngResponse | null>(null);
    const [loading, setLoading] = useState<boolean>(false);
    const [error, setError] = useState<string | null>(null);

    // Create stable request object for dependency array
    const requestKey = request
        ? JSON.stringify({
              sprite_path: request.sprite_path,
              act_path: request.act_path,
              action_index: request.action_index ?? 0,
              frame_index: request.frame_index ?? 0,
              palette_path: request.palette_path,
              scale: request.scale ?? 1.0,
          })
        : null;

    const loadSprite = useCallback(async () => {
        if (!request) {
            setSprite(null);
            setLoading(false);
            setError(null);
            return;
        }

        setLoading(true);
        setError(null);

        try {
            const fullRequest: SpritePngRequest = {
                sprite_path: request.sprite_path,
                act_path: request.act_path,
                action_index: request.action_index ?? 0,
                frame_index: request.frame_index ?? 0,
                palette_path: request.palette_path,
                scale: request.scale ?? 1.0,
            };

            const response = await getSpritePng(fullRequest);
            setSprite(response);
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : String(err);
            setError(errorMessage);
            setSprite(null);
        } finally {
            setLoading(false);
        }
    }, [requestKey]);

    useEffect(() => {
        loadSprite();
    }, [loadSprite]);

    const refetch = useCallback(() => {
        loadSprite();
    }, [loadSprite]);

    return {
        sprite,
        loading,
        error,
        refetch,
    };
}
