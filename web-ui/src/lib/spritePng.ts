import { invoke } from '@tauri-apps/api/core';

/**
 * Request parameters for sprite PNG generation
 */
export interface SpritePngRequest {
    /** Path to .spr file (e.g., "data\\sprite\\인간족\\몸통\\여\\여_body.spr") */
    sprite_path: string;
    /** Optional ACT file path (auto-inferred if not provided) */
    act_path?: string;
    /** Action index (0 = idle, 1 = walk, etc.) */
    action_index: number;
    /** Frame index within the action */
    frame_index: number;
    /** Optional custom palette file path for color variations (e.g., hair colors) */
    palette_path?: string;
    /** Scale factor for rendering (default: 1.0) */
    scale?: number;
}

/**
 * Response from sprite PNG generation
 */
export interface SpritePngResponse {
    /** Data URL in format: "data:image/png;base64,..." */
    data_url: string;
    /** Image width in pixels */
    width: number;
    /** Image height in pixels */
    height: number;
    /** X offset from ACT layer (for positioning relative to character anchor) */
    offset_x: number;
    /** Y offset from ACT layer (for positioning relative to character anchor) */
    offset_y: number;
    /** Whether this response came from cache */
    from_cache: boolean;
}

/**
 * Get or generate a sprite PNG for a specific action and frame
 *
 * This function leverages a 3-tier caching system:
 * 1. In-memory LRU cache (fastest)
 * 2. Disk cache (fast)
 * 3. Generation from SPR/ACT files (slowest)
 *
 * @param request Sprite request parameters
 * @returns Promise resolving to sprite PNG response with data URL
 * @throws Error if sprite generation fails
 *
 * @example
 * ```typescript
 * const sprite = await getSpritePng({
 *     sprite_path: 'data\\sprite\\인간족\\몸통\\여\\여_body.spr',
 *     action_index: 0,
 *     frame_index: 0,
 *     scale: 2.0
 * });
 * imageElement.src = sprite.data_url;
 * ```
 */
export async function getSpritePng(request: SpritePngRequest): Promise<SpritePngResponse> {
    try {
        const response = await invoke<SpritePngResponse>('get_sprite_png', {
            spritePath: request.sprite_path,
            actionIndex: request.action_index,
            frameIndex: request.frame_index,
            actPath: request.act_path,
            palettePath: request.palette_path,
            scale: request.scale ?? 1.0,
        });

        return response;
    } catch (error) {
        throw new Error(`Failed to get sprite PNG: ${error}`);
    }
}

/**
 * Response from batch preload operations
 */
export interface PreloadBatchResponse {
    /** Cache keys for successfully loaded sprites */
    successful_keys: string[];
    /** Cache keys for sprites that failed to load */
    failed_keys: string[];
    /** Total number of sprites requested */
    total: number;
}

/**
 * Preload a batch of sprites into the cache
 *
 * Useful for preloading UI sprites before they're needed,
 * reducing latency when switching between screens or showing animations.
 * Continues processing even if individual sprites fail to load.
 *
 * @param requests Array of sprite requests to preload
 * @returns Promise resolving to PreloadBatchResponse with success/failure details
 *
 * @example
 * ```typescript
 * const result = await preloadSpriteBatch([
 *     { sprite_path: 'sprite1.spr', action_index: 0, frame_index: 0 },
 *     { sprite_path: 'sprite2.spr', action_index: 0, frame_index: 0 },
 * ]);
 * console.log(`Preloaded ${result.successful_keys.length}/${result.total} sprites`);
 * if (result.failed_keys.length > 0) {
 *     console.warn(`Failed to load ${result.failed_keys.length} sprites`);
 * }
 * ```
 */
export async function preloadSpriteBatch(requests: SpritePngRequest[]): Promise<PreloadBatchResponse> {
    try {
        const response = await invoke<PreloadBatchResponse>('preload_sprite_batch', {
            requests: requests.map(req => ({
                sprite_path: req.sprite_path,
                act_path: req.act_path,
                action_index: req.action_index,
                frame_index: req.frame_index,
                palette_path: req.palette_path,
                scale: req.scale ?? 1.0,
            })),
        });

        return response;
    } catch (error) {
        throw new Error(`Failed to preload sprite batch: ${error}`);
    }
}

/**
 * Clear all sprite PNGs from the cache (both memory and disk)
 *
 * Useful for debugging or when asset files have been updated and
 * you need to regenerate cached sprites.
 *
 * @returns Promise that resolves when cache is cleared
 *
 * @example
 * ```typescript
 * await clearSpriteCache();
 * console.log('Sprite cache cleared');
 * ```
 */
export async function clearSpriteCache(): Promise<void> {
    try {
        await invoke<void>('clear_sprite_cache');
    } catch (error) {
        throw new Error(`Failed to clear sprite cache: ${error}`);
    }
}
