# Sprite PNG Quick Start Guide

## 🚀 TL;DR - Copy & Paste Examples

### Display a Single Sprite
```typescript
import { SpriteImage } from '@/components';

<SpriteImage
    spritePath="data\sprite\인간족\몸통\여\여_body.spr"
    actionIndex={0}
    frameIndex={0}
    scale={2.0}
/>
```

### Character with Hair Color
```typescript
import { CharacterPreview } from '@/components';

<CharacterPreview
    gender="female"
    hairStyle={1}
    hairColor={0}  // 0=red, 1=yellow, 2=violet, 3=green, 4=blue, 5=white, 6=black, 7=navy
    scale={2.0}
/>
```

### Load Multiple Sprites
```typescript
import { useSpritePngBatch } from '@/hooks';

const sprites = [
    { sprite_path: 'hair1.spr', action_index: 0, frame_index: 0 },
    { sprite_path: 'hair2.spr', action_index: 0, frame_index: 0 },
];

const { sprites: loaded, loading, progress } = useSpritePngBatch(sprites);

if (loading) return <div>Loading {progress.loaded}/{progress.total}...</div>;
```

### Clear Cache (Debugging)
```typescript
import { clearSpriteCache } from '@/lib/spritePng';

await clearSpriteCache();
```

## 📦 Import Paths

```typescript
// Components (high-level, recommended)
import { SpriteImage, CharacterPreview } from '@/components';

// Hooks (for custom logic)
import { useSpritePng, useSpritePngBatch } from '@/hooks';

// Low-level API (advanced usage)
import { getSpritePng, preloadSpriteBatch, clearSpriteCache } from '@/lib/spritePng';
import type { SpritePngRequest, SpritePngResponse } from '@/lib/spritePng';
```

## 🎯 Common Use Cases

### Case 1: Item Icon
```typescript
<SpriteImage
    spritePath={item.iconPath}
    actionIndex={0}
    frameIndex={0}
    scale={1.0}
    alt={item.name}
/>
```

### Case 2: Character Selection
```typescript
{characters.map(char => (
    <CharacterPreview
        key={char.id}
        gender={char.gender}
        hairStyle={char.hairStyle}
        hairColor={char.hairColor}
        actionIndex={0}
        scale={2.0}
    />
))}
```

### Case 3: Hair Style Picker
```typescript
const hairStyles = Array.from({ length: 28 }, (_, i) => ({
    sprite_path: `data\\sprite\\인간족\\머리통\\여\\여_머리_${String(i + 1).padStart(2, '0')}.spr`,
    action_index: 0,
    frame_index: 0,
}));

const { sprites, loading } = useSpritePngBatch(hairStyles);
```

### Case 4: Animated Sprite (Manual Frame Control)
```typescript
const [frame, setFrame] = useState(0);

useEffect(() => {
    const interval = setInterval(() => {
        setFrame(f => (f + 1) % 8); // 8 frames
    }, 100); // 100ms per frame
    return () => clearInterval(interval);
}, []);

<SpriteImage
    spritePath="animated.spr"
    actionIndex={1} // walking
    frameIndex={frame}
/>
```

### Case 5: Conditional Loading
```typescript
const { sprite, loading } = useSpritePng(
    showPreview ? {
        sprite_path: 'preview.spr',
        action_index: 0,
        frame_index: 0
    } : null  // Don't load when not visible
);
```

## 🎨 Styling Sprites

### Pixel-Perfect Rendering
```typescript
<SpriteImage
    spritePath="sprite.spr"
    style={{
        imageRendering: 'pixelated',  // Already applied by default
        imageRendering: 'crisp-edges', // Alternative for some browsers
    }}
/>
```

### Layering Sprites (Custom)
```typescript
<div style={{ position: 'relative' }}>
    {/* Layer 1: Body */}
    <SpriteImage
        spritePath="body.spr"
        style={{ position: 'absolute', top: 0, left: 0 }}
    />

    {/* Layer 2: Equipment */}
    <SpriteImage
        spritePath="weapon.spr"
        style={{ position: 'absolute', top: 0, left: 0 }}
    />
</div>
```

## ⚡ Performance Tips

### 1. Preload Before Showing
```typescript
// Preload on screen mount
useEffect(() => {
    preloadSpriteBatch([...sprites]);
}, []);
```

### 2. Use Consistent Scale
```typescript
// ✅ Good - Same scale = cache hits
<SpriteImage scale={2.0} />
<SpriteImage scale={2.0} />

// ❌ Bad - Different scales = separate cache entries
<SpriteImage scale={2.0} />
<SpriteImage scale={2.1} />
```

### 3. Batch Loading
```typescript
// ✅ Good - Parallel loading
useSpritePngBatch([sprite1, sprite2, sprite3]);

// ❌ Bad - Sequential loading
useSpritePng(sprite1);
useSpritePng(sprite2);
useSpritePng(sprite3);
```

## 🐛 Troubleshooting

### Sprite Not Loading?
1. Check file path (use backslashes: `data\sprite\...`)
2. Check if file exists in GRF or data folder
3. Check browser console for errors
4. Try clearing cache: `await clearSpriteCache()`

### Slow Loading?
1. First load is slowest (generates PNG)
2. Subsequent loads use cache (fast)
3. Use batch preloading for better UX

### Wrong Colors?
1. Check if sprite needs a palette file
2. Verify palette path matches sprite
3. Hair sprites require color palettes

### TypeScript Errors?
1. Import from barrel exports: `@/hooks`, `@/components`
2. Use proper types: `SpritePngRequest`, `SpritePngResponse`
3. Check required vs optional properties

## 📊 Default Values

| Parameter | Default | Notes |
|-----------|---------|-------|
| `actionIndex` | `0` | 0 = idle, 1 = walk, etc. |
| `frameIndex` | `0` | First frame |
| `scale` | `1.0` | Original size |
| `actPath` | `undefined` | Auto-inferred from sprite path |
| `palettePath` | `undefined` | No color variation |

## 🔗 Full Documentation

- **Complete API**: `/web-ui/SPRITE_PNG_INTEGRATION.md`
- **Test Screen**: `/web-ui/src/screens/SpritePngTest.tsx`
- **Examples**: `/web-ui/src/components/CharacterPreview.tsx`

## 🎮 Test It Out

```bash
# Start dev server
cd web-ui
npm run dev

# Then open SpritePngTest screen in the UI
# (Add route to access it)
```

---

Need help? Check the full docs or see the test screen for live examples!
