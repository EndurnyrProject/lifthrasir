import { CSSProperties } from 'react';
import { useSpritePng } from '../hooks/useSpritePng';

/**
 * Fine-tune vertical alignment for Ragnarok Online sprite centering.
 * This offset compensates for the difference between RO's coordinate system
 * (Y-negative=up) and CSS positioning (Y-positive=down).
 */
const Y_OFFSET_ADJUSTMENT = 4.5;

/**
 * Props for SpriteImage component
 */
export interface SpriteImageProps {
    /** Path to .spr file (e.g., "data\\sprite\\인간족\\몸통\\여\\여_body.spr") */
    spritePath: string;
    /** Optional ACT file path (auto-inferred if not provided) */
    actPath?: string;
    /** Action index (default: 0) */
    actionIndex?: number;
    /** Frame index (default: 0) */
    frameIndex?: number;
    /** Optional custom palette file path for color variations */
    palettePath?: string;
    /** Scale factor for rendering (default: 1.0) */
    scale?: number;
    /** Additional CSS class name */
    className?: string;
    /** Inline styles */
    style?: CSSProperties;
    /** Alt text for accessibility */
    alt?: string;
    /** Custom loading placeholder element */
    loadingPlaceholder?: React.ReactNode;
    /** Custom error placeholder element */
    errorPlaceholder?: React.ReactNode;
    /** Whether to apply ACT offsets (default: true) */
    applyOffset?: boolean;
}

/**
 * Reusable component for displaying Ragnarok Online sprites
 *
 * Automatically handles loading states, errors, and displays the sprite image
 * when ready. Uses the sprite PNG rendering system with 3-tier caching.
 *
 * @example
 * ```tsx
 * // Basic usage
 * <SpriteImage
 *     spritePath="data\\sprite\\인간족\\몸통\\여\\여_body.spr"
 *     actionIndex={0}
 *     frameIndex={0}
 * />
 * ```
 *
 * @example
 * ```tsx
 * // With custom palette and scale
 * <SpriteImage
 *     spritePath="data\\sprite\\인간족\\머리통\\여\\여_머리_01.spr"
 *     actionIndex={0}
 *     frameIndex={0}
 *     palettePath="data\\palette\\머리\\머리_01_빨강.pal"
 *     scale={2.0}
 *     alt="Red hair style 1"
 * />
 * ```
 *
 * @example
 * ```tsx
 * // With custom loading/error states
 * <SpriteImage
 *     spritePath="sprite.spr"
 *     loadingPlaceholder={<Spinner />}
 *     errorPlaceholder={<div>Failed to load sprite</div>}
 * />
 * ```
 */
export function SpriteImage({
    spritePath,
    actPath,
    actionIndex = 0,
    frameIndex = 0,
    palettePath,
    scale = 1.0,
    className,
    style,
    alt = 'Sprite',
    loadingPlaceholder,
    errorPlaceholder,
    applyOffset = true,
}: SpriteImageProps) {
    const { sprite, loading, error } = useSpritePng({
        sprite_path: spritePath,
        act_path: actPath,
        action_index: actionIndex,
        frame_index: frameIndex,
        palette_path: palettePath,
        scale,
    });

    // Loading state
    if (loading) {
        return (
            loadingPlaceholder ?? (
                <div
                    className={className}
                    style={{
                        ...style,
                        display: 'inline-flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        minWidth: '100px',
                        minHeight: '100px',
                        opacity: 0.5,
                    }}
                >
                    Loading...
                </div>
            )
        );
    }

    // Error state
    if (error) {
        return (
            errorPlaceholder ?? (
                <div
                    className={className}
                    style={{
                        ...style,
                        display: 'inline-flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        minWidth: '100px',
                        minHeight: '100px',
                        opacity: 0.3,
                    }}
                    title={error}
                >
                    Error
                </div>
            )
        );
    }

    // Sprite loaded successfully
    if (!sprite) {
        return null;
    }

    return (
        <img
            src={sprite.data_url}
            alt={alt}
            className={className}
            style={{
                ...style,
                width: sprite.width,
                height: sprite.height,
                // Apply ACT offset for sprite alignment with Y adjustment
                ...(applyOffset && {
                    marginLeft: sprite.offset_x,
                    marginTop: sprite.offset_y + Y_OFFSET_ADJUSTMENT,
                }),
            }}
        />
    );
}
