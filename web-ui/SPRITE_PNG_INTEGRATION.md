# Sprite PNG React Integration

This document describes the React integration layer for the static sprite PNG generation system (Phase 3).

## Overview

The React integration provides a complete TypeScript-safe interface for rendering Ragnarok Online sprites in the UI. It leverages the 3-tier caching system (in-memory LRU → disk cache → generation) implemented in Phases 1 and 2.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    React Components                      │
│  ┌──────────────┐  ┌────────────────────────────────┐  │
│  │ SpriteImage  │  │    CharacterPreview            │  │
│  └──────────────┘  └────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────┐
│                    Custom Hooks                          │
│  ┌──────────────┐  ┌────────────────────────────────┐  │
│  │useSpritePng  │  │    useSpritePngBatch           │  │
│  └──────────────┘  └────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────┐
│              Tauri Command Wrappers                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │getSpritePng  │  │ preloadBatch │  │ clearCache   │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────┐
│                   Tauri Backend                          │
│              (3-Tier Caching System)                     │
└─────────────────────────────────────────────────────────┘
```

## Files Created

### Core Library
- **`src/lib/spritePng.ts`**: TypeScript types and Tauri command wrappers
  - `SpritePngRequest` - Request parameters interface
  - `SpritePngResponse` - Response with data URL and metadata
  - `getSpritePng()` - Get/generate single sprite
  - `preloadSpriteBatch()` - Batch preload sprites
  - `clearSpriteCache()` - Clear all cached sprites

### Custom Hooks
- **`src/hooks/useSpritePng.ts`**: Single sprite loading hook
  - Auto-loads on mount
  - Re-fetches on parameter changes
  - Supports null request for conditional rendering
  - Returns: `{ sprite, loading, error, refetch }`

- **`src/hooks/useSpritePngBatch.ts`**: Batch sprite loading hook
  - Parallel loading with progress tracking
  - Continues on individual failures
  - Returns: `{ sprites: Map, loading, error, progress, refetch }`

- **`src/hooks/index.ts`**: Barrel export for hooks

### Components
- **`src/components/SpriteImage.tsx`**: Reusable sprite display component
  - Handles loading/error states
  - Supports all sprite parameters (action, frame, palette, scale)
  - Customizable loading/error placeholders

- **`src/components/CharacterPreview.tsx`**: Character layering example
  - Demonstrates body + hair layering
  - Shows palette usage for hair colors
  - Includes example component with multiple previews

- **`src/components/index.ts`**: Barrel export for components

### Test/Demo
- **`src/screens/SpritePngTest.tsx`**: Comprehensive test screen
  - Single sprite loading demo
  - Character preview with controls
  - Batch loading example
  - Cache management UI

## Usage Examples

### 1. Single Sprite with SpriteImage Component

```typescript
import { SpriteImage } from '../components';

function MyComponent() {
    return (
        <SpriteImage
            spritePath="data\sprite\인간족\몸통\여\여_body.spr"
            actionIndex={0}
            frameIndex={0}
            scale={2.0}
            alt="Female body"
        />
    );
}
```

### 2. Single Sprite with useSpritePng Hook

```typescript
import { useSpritePng } from '../hooks';

function MyComponent() {
    const { sprite, loading, error } = useSpritePng({
        sprite_path: 'data\sprite\인간족\몸통\여\여_body.spr',
        action_index: 0,
        frame_index: 0,
        scale: 2.0
    });

    if (loading) return <div>Loading...</div>;
    if (error) return <div>Error: {error}</div>;
    if (!sprite) return null;

    return <img src={sprite.data_url} alt="Character" />;
}
```

### 3. Character Preview with Layering

```typescript
import { CharacterPreview } from '../components';

function CharacterCustomizer() {
    const [hairStyle, setHairStyle] = useState(1);
    const [hairColor, setHairColor] = useState(0);

    return (
        <CharacterPreview
            gender="female"
            hairStyle={hairStyle}
            hairColor={hairColor}
            actionIndex={0}
            frameIndex={0}
            scale={2.0}
        />
    );
}
```

### 4. Batch Loading

```typescript
import { useSpritePngBatch } from '../hooks';

function HairColorSelector() {
    const hairColors = [
        { sprite_path: 'hair.spr', action_index: 0, frame_index: 0, palette_path: 'red.pal', scale: 1.5 },
        { sprite_path: 'hair.spr', action_index: 0, frame_index: 0, palette_path: 'blue.pal', scale: 1.5 },
        { sprite_path: 'hair.spr', action_index: 0, frame_index: 0, palette_path: 'green.pal', scale: 1.5 },
    ];

    const { sprites, loading, progress } = useSpritePngBatch(hairColors);

    if (loading) {
        return <div>Loading {progress.loaded}/{progress.total}...</div>;
    }

    return (
        <div>
            {hairColors.map((req, index) => {
                const key = JSON.stringify(req);
                const sprite = sprites.get(key);
                return sprite ? (
                    <img key={index} src={sprite.data_url} alt={`Color ${index}`} />
                ) : null;
            })}
        </div>
    );
}
```

### 5. Cache Management

```typescript
import { clearSpriteCache } from '../lib/spritePng';

async function clearCache() {
    try {
        await clearSpriteCache();
        console.log('Cache cleared');
    } catch (error) {
        console.error('Failed to clear cache:', error);
    }
}
```

## API Reference

### SpritePngRequest Interface

```typescript
interface SpritePngRequest {
    sprite_path: string;        // Path to .spr file
    act_path?: string;          // Optional ACT file (auto-inferred if not provided)
    action_index: number;       // Action index (0 = idle, 1 = walk, etc.)
    frame_index: number;        // Frame index within action
    palette_path?: string;      // Optional palette for color variations
    scale?: number;             // Scale factor (default: 1.0)
}
```

### SpritePngResponse Interface

```typescript
interface SpritePngResponse {
    data_url: string;           // "data:image/png;base64,..."
    width: number;              // Image width in pixels
    height: number;             // Image height in pixels
    from_cache: boolean;        // Whether from cache
}
```

### useSpritePng Hook

```typescript
function useSpritePng(request: UseSpritePngOptions | null): {
    sprite: SpritePngResponse | null;
    loading: boolean;
    error: string | null;
    refetch: () => void;
}
```

### useSpritePngBatch Hook

```typescript
function useSpritePngBatch(requests: SpritePngRequest[]): {
    sprites: Map<string, SpritePngResponse>;
    loading: boolean;
    error: string | null;
    progress: { loaded: number; total: number };
    refetch: () => void;
}
```

### SpriteImage Component Props

```typescript
interface SpriteImageProps {
    spritePath: string;
    actPath?: string;
    actionIndex?: number;
    frameIndex?: number;
    palettePath?: string;
    scale?: number;
    className?: string;
    style?: CSSProperties;
    alt?: string;
    loadingPlaceholder?: React.ReactNode;
    errorPlaceholder?: React.ReactNode;
}
```

### CharacterPreview Component Props

```typescript
interface CharacterPreviewProps {
    gender: 'male' | 'female';
    hairStyle: number;          // 1-28 for most styles
    hairColor: number;          // 0-7 for standard colors
    actionIndex?: number;
    frameIndex?: number;
    scale?: number;
    className?: string;
    style?: CSSProperties;
}
```

## Performance Characteristics

### Caching Behavior
1. **First Load**: Slowest (~50-200ms) - generates PNG from SPR/ACT files
2. **Disk Cache Hit**: Fast (~10-50ms) - loads from disk cache
3. **Memory Cache Hit**: Fastest (~1-5ms) - loads from LRU memory cache

### Memory Management
- **Data URLs**: No cleanup needed (self-contained base64 strings)
- **LRU Memory Cache**: Automatically evicts old entries (configurable size in backend)
- **Disk Cache**: Persistent across app restarts, cleared manually if needed

### Best Practices
1. **Use Batch Loading**: Preload sprites before they're needed
2. **Conditional Rendering**: Pass `null` to `useSpritePng` when not needed
3. **Stable References**: Use stable request objects to avoid unnecessary re-fetches
4. **Scale Factor**: Use consistent scale factors to maximize cache hits

## Testing

Run the test screen to verify functionality:

```bash
# Start the dev server
cd web-ui
npm run dev
```

Then navigate to the SpritePngTest screen in the UI to see:
- Single sprite loading
- Character preview with controls
- Batch loading examples
- Cache management UI

## Integration Points

### Character Selection Screen
Replace static images with `CharacterPreview` component:
```typescript
<CharacterPreview
    gender={selectedCharacter.gender}
    hairStyle={selectedCharacter.hair_style}
    hairColor={selectedCharacter.hair_color}
/>
```

### Character Creation Screen
Use `useSpritePngBatch` to preload hair style options:
```typescript
const hairStylePreviews = useMemo(() => {
    return Array.from({ length: 28 }, (_, i) => ({
        sprite_path: `data\\sprite\\인간족\\머리통\\여\\여_머리_${String(i + 1).padStart(2, '0')}.spr`,
        action_index: 0,
        frame_index: 0,
        scale: 1.5
    }));
}, []);

const { sprites, loading } = useSpritePngBatch(hairStylePreviews);
```

### Inventory UI
Use `SpriteImage` for item icons:
```typescript
<SpriteImage
    spritePath={item.sprite_path}
    actionIndex={0}
    frameIndex={0}
    scale={1.0}
    alt={item.name}
/>
```

## Troubleshooting

### Sprites Not Loading
1. **Check file paths**: Ensure paths match files in GRF or data folder
2. **Check ACT files**: Some sprites require ACT files for animation data
3. **Check console**: Look for Tauri command errors
4. **Clear cache**: Use `clearSpriteCache()` to force regeneration

### Performance Issues
1. **Reduce batch size**: Load sprites in smaller batches
2. **Increase scale**: Larger sprites take longer to generate
3. **Check disk space**: Disk cache requires available storage

### Type Errors
1. **Import from barrel exports**: Use `import { ... } from '../hooks'` not individual files
2. **Null checks**: Handle null sprite responses properly
3. **Stable dependencies**: Use `useMemo` for request objects if dynamically generated

## Future Enhancements

Potential improvements for future iterations:
1. **Animation Support**: Auto-advance frames for animated sprites
2. **Sprite Sheets**: Combine multiple frames into sprite sheets
3. **Virtual Scrolling**: Optimize rendering of large sprite lists
4. **Preload Strategies**: Intelligent preloading based on user navigation
5. **Compression**: Optional compression for data URLs
6. **Service Worker**: Cache sprites in browser service worker

## Related Documentation

- **Phase 1**: `game-engine/src/infrastructure/sprite_png/README.md` - Headless PNG renderer
- **Phase 2**: `src-tauri/src/commands/sprite_png.rs` - Tauri commands
- **Project Docs**: `CLAUDE.md` - Overall project documentation
