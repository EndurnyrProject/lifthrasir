import { invoke } from '@tauri-apps/api/core';
import { useState, useEffect, useRef } from 'react';
import { SpriteImage } from '../components';
import { Gender, getBodySpritePath, getHairSpritePath, getHairPalettePath } from '../lib/characterSprites';
import './CharacterCreation.css';

interface HairstyleInfo {
  id: number;
  available_colors: number[];
}

interface HairstylesResponse {
  success: boolean;
  error?: string;
  hairstyles?: HairstyleInfo[];
}

interface CharacterCreationProps {
  selectedSlot: number;
  onCharacterCreated: () => void;
  onCancel: () => void;
}

export default function CharacterCreation({
  selectedSlot,
  onCharacterCreated,
  onCancel
}: CharacterCreationProps) {
  // Form state
  const [characterName, setCharacterName] = useState('');
  const [selectedGender, setSelectedGender] = useState<Gender>(Gender.Male);
  const [selectedHairStyle, setSelectedHairStyle] = useState(1);
  const [selectedHairColor, setSelectedHairColor] = useState(0);

  // UI state
  const [hairstyles, setHairstyles] = useState<HairstyleInfo[]>([]);
  const [availableColors, setAvailableColors] = useState<number[]>([0]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const assetsLoadedRef = useRef(false);

  useEffect(() => {
    if (assetsLoadedRef.current) {
      return;
    }
    assetsLoadedRef.current = true;

    const loadAssets = async () => {
      try {
        await loadHairstyles(selectedGender);
      } catch (err) {
        setError('Failed to load assets');
      }
    };

    loadAssets();
  }, []);

  const loadHairstyles = async (gender: Gender) => {
    try {
      const result = await invoke<HairstylesResponse>('get_hairstyles', { gender });

      if (result.success && result.hairstyles) {
        setHairstyles(result.hairstyles);

        // Select first hairstyle if available
        if (result.hairstyles.length > 0) {
          const firstStyle = result.hairstyles[0];
          setSelectedHairStyle(firstStyle.id);
          setAvailableColors(firstStyle.available_colors);
          setSelectedHairColor(firstStyle.available_colors[0] || 0);
        }
      } else {
        setError(result.error || 'Failed to load hairstyles');
      }
    } catch (err) {
      setError('Network error: ' + err);
    }
  };

  const handleGenderChange = async (gender: Gender) => {
    setSelectedGender(gender);
    setLoading(true);
    await loadHairstyles(gender);
    setLoading(false);
  };

  const handleHairstyleSelect = (styleInfo: HairstyleInfo) => {
    setSelectedHairStyle(styleInfo.id);
    setAvailableColors(styleInfo.available_colors);

    const newColor = styleInfo.available_colors[0] || 0;
    setSelectedHairColor(newColor);
  };

  const handleHairColorSelect = (color: number) => {
    setSelectedHairColor(color);
  };

  const handleCreateCharacter = async () => {
    if (!characterName.trim()) {
      setError('Please enter a character name');
      return;
    }

    if (characterName.length < 4) {
      setError('Character name must be at least 4 characters');
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const result = await invoke<{ success: boolean; error?: string }>('create_character', {
        request: {
          name: characterName,
          slot: selectedSlot,
          hair_style: selectedHairStyle,
          hair_color: selectedHairColor,
          sex: selectedGender
        }
      });

      if (result.success) {
        onCharacterCreated();
      } else {
        setError(result.error || 'Character creation failed');
      }
    } catch (err) {
      setError('Network error: ' + err);
    } finally {
      setLoading(false);
    }
  };

  // Show simple loading indicator while hairstyles load
  if (hairstyles.length === 0 && !loading && !error) {
    return (
      <div className="character-creation-container">
        <div className="customization-panel">
          <h1>Loading...</h1>
        </div>
      </div>
    );
  }

  return (
    <div className="character-creation-container">

      <div className="character-preview-container">
        <div className="character-sprite-preview">
          {/* Body sprite - Novice class (0) - no offset, this is the anchor */}
          <SpriteImage
            spritePath={getBodySpritePath(0, selectedGender)}
            actionIndex={0}
            frameIndex={0}
            scale={1.5}
            className="character-body-sprite"
            alt="Character body"
            applyOffset={false}
          />

          {/* Hair sprite with palette - offset applied for alignment */}
          <SpriteImage
            spritePath={getHairSpritePath(selectedHairStyle, selectedGender)}
            actionIndex={0}
            frameIndex={0}
            palettePath={getHairPalettePath(
              selectedHairStyle,
              selectedGender,
              selectedHairColor
            )}
            scale={1.5}
            className="character-hair-sprite"
            alt="Character hair"
          />
        </div>
      </div>

      <div className="customization-panel">
        <h1>Create Character</h1>

        <div className="input-group">
          <label htmlFor="char-name">Character Name</label>
          <input
            id="char-name"
            type="text"
            value={characterName}
            onChange={(e) => setCharacterName(e.target.value)}
            maxLength={23}
            placeholder="Enter character name"
            disabled={loading}
          />
          <span className="input-hint">4-23 characters, alphanumeric only</span>
        </div>

        <div className="gender-selection">
          <label>Gender</label>
          <div className="gender-buttons">
            <button
              onClick={() => handleGenderChange(Gender.Male)}
              className={selectedGender === Gender.Male ? 'selected' : ''}
              disabled={loading}
            >
              Male
            </button>
            <button
              onClick={() => handleGenderChange(Gender.Female)}
              className={selectedGender === Gender.Female ? 'selected' : ''}
              disabled={loading}
            >
              Female
            </button>
          </div>
        </div>

        <div className="hairstyle-selection">
          <label>Hairstyle</label>
          <div className="hairstyle-grid">
            {hairstyles.map((style) => (
              <button
                key={style.id}
                onClick={() => handleHairstyleSelect(style)}
                className={`hairstyle-item ${selectedHairStyle === style.id ? 'selected' : ''}`}
                disabled={loading}
              >
                <span className="hairstyle-id">#{style.id}</span>
              </button>
            ))}
          </div>
        </div>

        {availableColors.length > 1 && (
          <div className="hair-color-selection">
            <label>Hair Color</label>
            <div className="color-grid">
              {availableColors.map((color) => (
                <button
                  key={color}
                  onClick={() => handleHairColorSelect(color)}
                  className={`color-item ${selectedHairColor === color ? 'selected' : ''}`}
                  disabled={loading}
                >
                  <span className="color-id">#{color}</span>
                </button>
              ))}
            </div>
          </div>
        )}

        {error && <div className="error-message">{error}</div>}

        <div className="buttons-container">
          <button
            onClick={onCancel}
            className="cancel-button"
            disabled={loading}
          >
            Cancel
          </button>
          <button
            onClick={handleCreateCharacter}
            className="create-button"
            disabled={loading || !characterName.trim()}
          >
            {loading ? 'Creating...' : 'Create Character'}
          </button>
        </div>
      </div>
    </div>
  );
}
