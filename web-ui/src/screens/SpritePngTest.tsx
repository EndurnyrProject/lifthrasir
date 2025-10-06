import { useState } from 'react';
import { SpriteImage, CharacterPreview } from '../components';
import { useSpritePngBatch } from '../hooks';
import { clearSpriteCache } from '../lib/spritePng';
import { Gender } from '../lib/characterSprites';

/**
 * Test screen for sprite PNG rendering system
 *
 * Demonstrates:
 * - Single sprite loading with SpriteImage
 * - Character layering with CharacterPreview
 * - Batch loading with useSpritePngBatch
 * - Cache management
 */
export default function SpritePngTest() {
    const [hairStyle, setHairStyle] = useState(1);
    const [hairColor, setHairColor] = useState(0);
    const [clearing, setClearing] = useState(false);

    // Example: Batch load multiple sprites for a selection UI
    const hairColorOptions = [
        { sprite_path: 'data\\sprite\\인간족\\머리통\\여\\여_머리_01.spr', action_index: 0, frame_index: 0, palette_path: 'data\\palette\\머리\\머리_01_빨강.pal', scale: 1.5 },
        { sprite_path: 'data\\sprite\\인간족\\머리통\\여\\여_머리_01.spr', action_index: 0, frame_index: 0, palette_path: 'data\\palette\\머리\\머리_01_노랑.pal', scale: 1.5 },
        { sprite_path: 'data\\sprite\\인간족\\머리통\\여\\여_머리_01.spr', action_index: 0, frame_index: 0, palette_path: 'data\\palette\\머리\\머리_01_보라.pal', scale: 1.5 },
        { sprite_path: 'data\\sprite\\인간족\\머리통\\여\\여_머리_01.spr', action_index: 0, frame_index: 0, palette_path: 'data\\palette\\머리\\머리_01_초록.pal', scale: 1.5 },
    ];

    const { sprites, loading: batchLoading, progress } = useSpritePngBatch(hairColorOptions);

    const handleClearCache = async () => {
        setClearing(true);
        try {
            await clearSpriteCache();
            alert('Cache cleared successfully!');
        } catch (err) {
            alert(`Failed to clear cache: ${err}`);
        } finally {
            setClearing(false);
        }
    };

    return (
        <div style={{ padding: '20px', fontFamily: 'Arial, sans-serif' }}>
            <h1>Sprite PNG Rendering System Test</h1>

            {/* Section 1: Single Sprite */}
            <section style={{ marginBottom: '40px', borderBottom: '1px solid #ccc', paddingBottom: '20px' }}>
                <h2>1. Single Sprite Loading</h2>
                <p>Using SpriteImage component to load a single sprite:</p>
                <SpriteImage
                    spritePath="data\sprite\인간족\몸통\여\여_body.spr"
                    actionIndex={0}
                    frameIndex={0}
                    scale={2.0}
                    alt="Female body"
                />
            </section>

            {/* Section 2: Character Preview */}
            <section style={{ marginBottom: '40px', borderBottom: '1px solid #ccc', paddingBottom: '20px' }}>
                <h2>2. Character Preview (Layered Sprites)</h2>
                <p>Demonstrates layering body + hair with custom palette:</p>

                <div style={{ marginBottom: '20px' }}>
                    <label style={{ display: 'block', marginBottom: '10px' }}>
                        Hair Style:
                        <input
                            type="number"
                            min="1"
                            max="28"
                            value={hairStyle}
                            onChange={(e) => setHairStyle(Number(e.target.value))}
                            style={{ marginLeft: '10px', padding: '5px' }}
                        />
                    </label>

                    <label style={{ display: 'block', marginBottom: '10px' }}>
                        Hair Color:
                        <select
                            value={hairColor}
                            onChange={(e) => setHairColor(Number(e.target.value))}
                            style={{ marginLeft: '10px', padding: '5px' }}
                        >
                            <option value={0}>Red (빨강)</option>
                            <option value={1}>Yellow (노랑)</option>
                            <option value={2}>Violet (보라)</option>
                            <option value={3}>Green (초록)</option>
                            <option value={4}>Blue (파랑)</option>
                            <option value={5}>White (흰색)</option>
                            <option value={6}>Black (검정)</option>
                            <option value={7}>Navy (남색)</option>
                        </select>
                    </label>
                </div>

                <CharacterPreview
                    gender={Gender.Female}
                    hairStyle={hairStyle}
                    hairColor={hairColor}
                    scale={2.0}
                />
            </section>

            {/* Section 3: Batch Loading */}
            <section style={{ marginBottom: '40px', borderBottom: '1px solid #ccc', paddingBottom: '20px' }}>
                <h2>3. Batch Loading</h2>
                <p>Preloading multiple sprites for a selection UI:</p>

                {batchLoading ? (
                    <div>Loading batch: {progress.loaded}/{progress.total} sprites...</div>
                ) : (
                    <div style={{ display: 'flex', gap: '10px', flexWrap: 'wrap' }}>
                        {hairColorOptions.map((req, index) => {
                            const key = JSON.stringify(req);
                            const sprite = sprites.get(key);
                            return (
                                <div key={index} style={{ textAlign: 'center' }}>
                                    {sprite ? (
                                        <img
                                            src={sprite.data_url}
                                            alt={`Hair color ${index}`}
                                            style={{ display: 'block', imageRendering: 'pixelated' }}
                                        />
                                    ) : (
                                        <div style={{ width: '64px', height: '64px', background: '#eee' }}>
                                            Failed
                                        </div>
                                    )}
                                    <div>Color {index + 1}</div>
                                </div>
                            );
                        })}
                    </div>
                )}
            </section>

            {/* Section 4: Cache Management */}
            <section style={{ marginBottom: '40px' }}>
                <h2>4. Cache Management</h2>
                <p>Clear all cached sprites (useful for debugging or asset updates):</p>
                <button
                    onClick={handleClearCache}
                    disabled={clearing}
                    style={{
                        padding: '10px 20px',
                        fontSize: '16px',
                        cursor: clearing ? 'not-allowed' : 'pointer',
                        background: clearing ? '#ccc' : '#e74c3c',
                        color: 'white',
                        border: 'none',
                        borderRadius: '4px',
                    }}
                >
                    {clearing ? 'Clearing...' : 'Clear Sprite Cache'}
                </button>
            </section>

            {/* Info Section */}
            <section style={{ background: '#f5f5f5', padding: '15px', borderRadius: '4px' }}>
                <h3>System Features</h3>
                <ul>
                    <li><strong>3-Tier Caching:</strong> In-memory LRU → Disk cache → Generation</li>
                    <li><strong>Automatic Loading:</strong> Sprites load on mount and re-fetch on parameter changes</li>
                    <li><strong>Error Handling:</strong> Graceful error states with fallback UI</li>
                    <li><strong>Batch Support:</strong> Parallel loading with progress tracking</li>
                    <li><strong>Palette Support:</strong> Custom color variations (hair colors, etc.)</li>
                    <li><strong>Data URLs:</strong> No memory cleanup needed (vs Blob URLs)</li>
                </ul>
            </section>
        </div>
    );
}
