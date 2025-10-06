import { CSSProperties } from 'react';
import { SpriteImage } from './SpriteImage';
import { Gender, getBodySpritePath, getHairSpritePath, getHairPalettePath } from '../lib/characterSprites';

/**
 * Props for CharacterPreview component
 */
export interface CharacterPreviewProps {
    /** Character gender (0 = Female, 1 = Male) */
    gender: Gender;
    /** Hair style index (1-28 for most styles) */
    hairStyle: number;
    /** Hair color index (0-7 for most colors) */
    hairColor: number;
    /** Job class (default: 0 for Novice) */
    jobClass?: number;
    /** Action index (default: 0 for idle) */
    actionIndex?: number;
    /** Frame index (default: 0) */
    frameIndex?: number;
    /** Scale factor (default: 2.0 for better visibility) */
    scale?: number;
    /** Additional CSS class name */
    className?: string;
    /** Inline styles for the container */
    style?: CSSProperties;
}

/**
 * Example component demonstrating character sprite layering
 *
 * Shows how to layer multiple sprites (body + hair) with custom palettes
 * to create a complete character preview. This pattern is used throughout
 * the character customization system.
 *
 * @example
 * ```tsx
 * // Female character with red hair
 * <CharacterPreview
 *     gender={Gender.Female}
 *     hairStyle={1}
 *     hairColor={0}
 *     scale={2.0}
 * />
 * ```
 *
 * @example
 * ```tsx
 * // Male character with different animation
 * <CharacterPreview
 *     gender={Gender.Male}
 *     hairStyle={5}
 *     hairColor={3}
 *     actionIndex={1}  // Walking animation
 *     frameIndex={0}
 * />
 * ```
 */
export function CharacterPreview({
    gender,
    hairStyle,
    hairColor,
    jobClass = 0,
    actionIndex = 0,
    frameIndex = 0,
    scale = 2.0,
    className,
    style,
}: CharacterPreviewProps) {
    // Use shared utility functions for consistent path generation
    const bodySpritePath = getBodySpritePath(jobClass, gender);
    const hairSpritePath = getHairSpritePath(hairStyle, gender);
    const hairPalettePath = getHairPalettePath(hairStyle, gender, hairColor);

    return (
        <div
            className={className}
            style={{
                ...style,
                position: 'relative',
                display: 'inline-block',
            }}
        >
            {/* Layer 1: Body (base layer) */}
            <SpriteImage
                spritePath={bodySpritePath}
                actionIndex={actionIndex}
                frameIndex={frameIndex}
                scale={scale}
                alt={`${gender} body`}
                style={{
                    position: 'absolute',
                    top: 0,
                    left: 0,
                    display: 'none'
                }}
            />

            {/* Layer 2: Hair (overlaid on body) */}
            <SpriteImage
                spritePath={hairSpritePath}
                palettePath={hairPalettePath}
                actionIndex={actionIndex}
                frameIndex={frameIndex}
                scale={scale}
                alt={`Hair style ${hairStyle} color ${hairColor}`}
                style={{
                    position: 'absolute',
                    top: 0,
                    left: 0,
                    zIndex: 1,
                }}
            />

            {/* Spacer to ensure container has the right dimensions */}
            {/* This prevents the absolutely positioned images from collapsing the container */}
            <div
                style={{
                    width: 64 * scale, // Typical sprite width
                    height: 64 * scale, // Typical sprite height
                    visibility: 'hidden',
                }}
            />
        </div>
    );
}

/**
 * Example usage component showing multiple character previews
 *
 * Demonstrates how to use CharacterPreview in a character selection
 * or customization screen.
 */
export function CharacterPreviewExample() {
    return (
        <div style={{ padding: '20px', display: 'flex', gap: '20px', flexWrap: 'wrap' }}>
            <div>
                <h3>Female - Red Hair</h3>
                <CharacterPreview gender={Gender.Female} hairStyle={1} hairColor={0} />
            </div>

            <div>
                <h3>Female - Blue Hair</h3>
                <CharacterPreview gender={Gender.Female} hairStyle={2} hairColor={4} />
            </div>

            <div>
                <h3>Male - Black Hair</h3>
                <CharacterPreview gender={Gender.Male} hairStyle={1} hairColor={6} />
            </div>

            <div>
                <h3>Male - Yellow Hair</h3>
                <CharacterPreview gender={Gender.Male} hairStyle={3} hairColor={1} />
            </div>
        </div>
    );
}
