# Sprite PNG Generation System

Headless PNG generation from Ragnarok Online sprite files (SPR + ACT) with three-tier caching.

## Quick Start

```rust
use game_engine::infrastructure::{
    assets::hierarchical_manager::HierarchicalAssetManager,
    sprite_png::{
        cache::SpritePngCache,
        renderer::SpriteRenderer,
        types::SpritePngRequest,
    },
};
use std::{path::PathBuf, sync::Arc};

// Initialize
let asset_manager = HierarchicalAssetManager::new();
let renderer = Arc::new(SpriteRenderer::new(asset_manager));
let cache = SpritePngCache::new(renderer, PathBuf::from(".cache/sprites"), 100)?;

// Request sprite
let request = SpritePngRequest {
    sprite_path: "data/sprite/몬스터/포링.spr".to_string(),
    act_path: None, // Auto-inferred
    action_index: 0,
    frame_index: 0,
    palette_path: None,
    scale: 1.0,
};

// Get PNG (cached when possible)
let response = cache.get_or_generate(&request)?;
let base64 = response.to_base64(); // For web transmission
```

## Architecture

### Three-Tier Cache

```
Request → [Memory LRU] → [Disk Cache] → [Generator] → Response
           ~1μs           ~1-5ms         ~10-50ms
```

1. **Memory (LRU)**: Hot sprites, instant access
2. **Disk**: Warm sprites, fast I/O
3. **Generator**: Cold sprites, on-demand rendering

### Components

- **types.rs**: Request/response structures
- **error.rs**: Comprehensive error types
- **renderer.rs**: Headless PNG generator
- **cache.rs**: Three-tier caching system

## API Reference

### SpritePngRequest

```rust
pub struct SpritePngRequest {
    pub sprite_path: String,        // "data/sprite/..."
    pub act_path: Option<String>,   // Auto-inferred if None
    pub action_index: usize,         // 0 = idle, 1 = walk, etc.
    pub frame_index: usize,          // Frame in animation
    pub palette_path: Option<String>, // Custom palette (hair colors)
    pub scale: f32,                  // 1.0 = original, 2.0 = 2x
}
```

**Methods:**
- `cache_key() -> String` - Generate unique cache key
- `get_act_path() -> String` - Get ACT path (inferred or explicit)

### SpritePngResponse

```rust
pub struct SpritePngResponse {
    pub png_data: Vec<u8>,  // PNG-encoded bytes
    pub width: u32,          // Image width
    pub height: u32,         // Image height
    pub from_cache: bool,    // Cache hit indicator
}
```

**Methods:**
- `to_base64() -> String` - Convert to base64 for web

### SpriteRenderer

```rust
pub struct SpriteRenderer {
    asset_manager: HierarchicalAssetManager,
}
```

**Methods:**
- `new(asset_manager) -> Self` - Create renderer
- `render_to_png(&self, request) -> Result<SpritePngResponse, SpritePngError>` - Generate PNG

### SpritePngCache

```rust
pub struct SpritePngCache {
    memory_cache: Arc<Mutex<LruCache<String, SpritePngResponse>>>,
    cache_dir: PathBuf,
    metadata: Arc<Mutex<CacheMetadata>>,
    renderer: Arc<SpriteRenderer>,
}
```

**Methods:**
- `new(renderer, cache_dir, capacity) -> Result<Self, SpritePngError>` - Create cache
- `get_or_generate(&self, request) -> Result<SpritePngResponse, SpritePngError>` - Get/generate PNG
- `preload_batch(&self, requests) -> Result<Vec<SpritePngResponse>, SpritePngError>` - Batch load
- `clear_all(&self) -> Result<(), SpritePngError>` - Clear all caches
- `get_stats(&self) -> Result<CacheStats, SpritePngError>` - Cache statistics

### SpritePngError

```rust
pub enum SpritePngError {
    FileNotFound(String),
    ParseError(String),
    InvalidAction(usize),
    InvalidFrame(usize),
    InvalidSpriteIndex(usize),
    NoLayers,
    ImageCreationFailed,
    InvalidPalette,
    EncodingError(String),
    CacheError(String),
    Io(std::io::Error),
    Image(image::ImageError),
    AssetSource(AssetSourceError),
    Sprite(SpriteError),
    Act(ActError),
    Json(serde_json::Error),
}
```

## Usage Examples

### Basic Sprite

```rust
let request = SpritePngRequest {
    sprite_path: "data/sprite/인간족/몸통/남/남_body.spr".to_string(),
    act_path: None,
    action_index: 0,
    frame_index: 0,
    palette_path: None,
    scale: 1.0,
};

let response = cache.get_or_generate(&request)?;
```

### Custom Palette (Hair Color)

```rust
let request = SpritePngRequest {
    sprite_path: "data/sprite/인간족/머리통/여/여_머리_1.spr".to_string(),
    act_path: None,
    action_index: 0,
    frame_index: 0,
    palette_path: Some("data/palette/머리/머리_1_1.pal".to_string()),
    scale: 1.0,
};

let response = cache.get_or_generate(&request)?;
```

### Scaled Sprite (UI Display)

```rust
let request = SpritePngRequest {
    sprite_path: "data/sprite/아이템/검.spr".to_string(),
    act_path: None,
    action_index: 0,
    frame_index: 0,
    palette_path: None,
    scale: 2.0, // 2x size for UI
};

let response = cache.get_or_generate(&request)?;
```

### Batch Preload

```rust
let requests = vec![
    SpritePngRequest { /* ... */ },
    SpritePngRequest { /* ... */ },
    SpritePngRequest { /* ... */ },
];

let responses = cache.preload_batch(&requests)?;
```

### Cache Statistics

```rust
let stats = cache.get_stats()?;
println!("Memory: {} sprites", stats.memory_count);
println!("Disk: {} sprites", stats.disk_count);
println!("Size: {} bytes", stats.total_size_bytes);
```

## Tauri Integration

```rust
use tauri::State;

#[tauri::command]
async fn get_sprite_png(
    sprite_path: String,
    action_index: usize,
    frame_index: usize,
    cache: State<'_, SpritePngCache>,
) -> Result<String, String> {
    let request = SpritePngRequest {
        sprite_path,
        act_path: None,
        action_index,
        frame_index,
        palette_path: None,
        scale: 1.0,
    };

    cache.get_or_generate(&request)
        .map(|r| r.to_base64())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn preload_character_sprites(
    cache: State<'_, SpritePngCache>,
) -> Result<(), String> {
    let requests = vec![
        // Body sprites
        SpritePngRequest { /* ... */ },
        // Hair sprites
        SpritePngRequest { /* ... */ },
        // Equipment sprites
        SpritePngRequest { /* ... */ },
    ];

    cache.preload_batch(&requests)
        .map(|_| ())
        .map_err(|e| e.to_string())
}
```

## React Integration

```typescript
// In React component
import { invoke } from '@tauri-apps/api/tauri';

async function loadSprite(
  spritePath: string,
  actionIndex: number,
  frameIndex: number
): Promise<string> {
  return await invoke('get_sprite_png', {
    spritePath,
    actionIndex,
    frameIndex
  });
}

// Usage
function CharacterPreview() {
  const [spriteData, setSpriteData] = useState<string>('');

  useEffect(() => {
    loadSprite('data/sprite/인간족/몸통/남/남_body.spr', 0, 0)
      .then(setSpriteData);
  }, []);

  return (
    <img
      src={`data:image/png;base64,${spriteData}`}
      alt="Character"
    />
  );
}
```

## Performance

### Memory Usage
- ~5KB per cached sprite
- 100 sprite capacity = ~500KB memory
- Configurable capacity

### Speed Benchmarks
- Memory hit: ~1μs (instant)
- Disk hit: ~1-5ms (I/O)
- Fresh generation: ~10-50ms (depends on complexity)

### Disk Usage
- PNG: ~1-10KB per sprite
- Metadata: ~100 bytes per sprite
- Total: ~10MB for 1000 sprites

## Error Handling

```rust
match cache.get_or_generate(&request) {
    Ok(response) => {
        // Success
        let base64 = response.to_base64();
    }
    Err(SpritePngError::FileNotFound(path)) => {
        eprintln!("Sprite not found: {}", path);
    }
    Err(SpritePngError::InvalidAction(idx)) => {
        eprintln!("Invalid action index: {}", idx);
    }
    Err(e) => {
        eprintln!("Error: {}", e);
    }
}
```

## Testing

```bash
# Run all tests
cargo test --lib sprite_png

# Run with output
cargo test --lib sprite_png -- --nocapture

# Run specific test
cargo test --lib sprite_png::cache::tests::test_cache_creation
```

## Example

See `game-engine/examples/sprite_png_generation.rs` for a comprehensive example:

```bash
cargo run --example sprite_png_generation
```

## Technical Details

### Path Handling
- Supports both Windows (`data\\sprite\\...`) and Unix (`data/sprite/...`) paths
- Auto-normalizes to forward slashes internally

### Palette Format
- 1024 bytes = 256 colors × 4 bytes (RGBA)
- Loaded from `.pal` files
- Applied during RGBA conversion

### Transparency
- Index 0 = transparent
- Magenta (255, 0, 255) = transparent
- Handled automatically

### Scaling
- Nearest neighbor filter (pixel-perfect)
- No blurring or artifacts
- Ideal for pixel art

### Cache Key Generation
- SHA-256 hash of request parameters
- Deterministic (same input = same key)
- f32 scale handled via `to_bits()`

## Limitations

1. **Single Layer**: Currently extracts first layer only
2. **Static Frames**: No animation support (individual frames only)
3. **No Composition**: Multi-layer composition not supported yet

## Future Enhancements

- Multi-layer composition
- Sprite sheet generation
- GIF animation export
- Dynamic client-side scaling
- WebP format support

## See Also

- `/Users/ygorcastor/Development/personal/lifthrasir/PHASE_1_IMPLEMENTATION_SUMMARY.md`
- `game-engine/examples/sprite_png_generation.rs`
- Parent documentation: `CLAUDE.md`
