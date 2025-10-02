import { invoke } from '@tauri-apps/api/core';

/**
 * Load an asset from the Ragnarok Online hierarchical asset system
 *
 * This function respects the configured asset hierarchy:
 * 1. Data folder (priority 0)
 * 2. GRF files (by configured priority)
 *
 * @param path Asset path using forward or backslashes (e.g., "data/texture/login.bmp" or "data\\texture\\login.bmp")
 * @returns Promise resolving to a Blob URL that can be used as img src
 * @throws Error if asset cannot be loaded
 *
 * @example
 * ```typescript
 * const bgUrl = await loadAsset('data/texture/유저인터페이스/login.bmp');
 * document.getElementById('bg').src = bgUrl;
 * ```
 */
export async function loadAsset(path: string): Promise<string> {
    try {
        // Call Rust backend to get asset bytes
        const bytes = await invoke<number[]>('get_asset', { path });

        // Convert number array to Uint8Array
        const uint8Array = new Uint8Array(bytes);

        // Create a Blob from the bytes
        const blob = new Blob([uint8Array]);

        // Create and return a Blob URL
        return URL.createObjectURL(blob);
    } catch (error) {
        throw new Error(`Failed to load asset '${path}': ${error}`);
    }
}

/**
 * Load an asset as a base64 data URL
 *
 * Alternative to loadAsset() that returns a data URL instead of a Blob URL.
 * Data URLs are self-contained but larger than Blob URLs.
 *
 * @param path Asset path
 * @param mimeType MIME type for the data URL (default: 'application/octet-stream')
 * @returns Promise resolving to a base64 data URL
 *
 * @example
 * ```typescript
 * const bgUrl = await loadAssetAsDataUrl('data/texture/login.bmp', 'image/bmp');
 * imageElement.src = bgUrl;
 * ```
 */
export async function loadAssetAsDataUrl(
    path: string,
    mimeType: string = 'application/octet-stream'
): Promise<string> {
    try {
        const bytes = await invoke<number[]>('get_asset', { path });

        // Convert to base64
        const base64 = btoa(String.fromCharCode(...bytes));

        return `data:${mimeType};base64,${base64}`;
    } catch (error) {
        throw new Error(`Failed to load asset '${path}': ${error}`);
    }
}

/**
 * Preload multiple assets concurrently
 *
 * Useful for loading multiple assets at application startup
 *
 * @param paths Array of asset paths to load
 * @returns Promise resolving to a Map of path → Blob URL
 *
 * @example
 * ```typescript
 * const assets = await preloadAssets([
 *     'data/texture/login_bg.bmp',
 *     'data/texture/button.bmp',
 * ]);
 * bgElement.src = assets.get('data/texture/login_bg.bmp')!;
 * ```
 */
export async function preloadAssets(paths: string[]): Promise<Map<string, string>> {
    const loadPromises = paths.map(async (path) => {
        const url = await loadAsset(path);
        return [path, url] as [string, string];
    });

    const results = await Promise.all(loadPromises);
    return new Map(results);
}

/**
 * Revoke a Blob URL to free memory
 *
 * Call this when you're done with an asset loaded via loadAsset()
 *
 * @param url Blob URL to revoke
 *
 * @example
 * ```typescript
 * const url = await loadAsset('data/texture/temp.bmp');
 * // ... use the URL ...
 * revokeAssetUrl(url); // Free memory
 * ```
 */
export function revokeAssetUrl(url: string): void {
    URL.revokeObjectURL(url);
}
