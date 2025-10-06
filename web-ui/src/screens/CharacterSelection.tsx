import { invoke } from '@tauri-apps/api/core';
import { useState, useEffect } from 'react';
import { SpriteImage } from '../components';
import { Gender, getBodySpritePath, getHairSpritePath, getHairPalettePath } from '../lib/characterSprites';
import { useAssets } from '../contexts/AssetsContext';
import CharacterCreation from './CharacterCreation';
import './CharacterSelection.css';

// Re-export Gender for backward compatibility
export { Gender };

// Job class enum matching Rust JobClass type
enum JobClass {
  Novice = 0,
  Swordsman = 1,
  Magician = 2,
  Archer = 3,
  Acolyte = 4,
  Merchant = 5,
  Thief = 6,
  Knight = 7,
  Priest = 8,
  Wizard = 9,
  Blacksmith = 10,
  Hunter = 11,
  Assassin = 12,
  Crusader = 14,
  Monk = 15,
  Sage = 16,
  Rogue = 17,
  Alchemist = 18,
  BardDancer = 19,
}

interface CharacterData {
  char_id: number;
  name: string;
  class: JobClass;
  base_level: number;
  job_level: number;
  base_exp: number;
  job_exp: number;
  hp: number;
  max_hp: number;
  sp: number;
  max_sp: number;
  zeny: number;
  str: number;
  agi: number;
  vit: number;
  int: number;
  dex: number;
  luk: number;
  hair_style: number;
  hair_color: number;
  clothes_color: number;
  weapon: number;
  shield: number;
  head_top: number;
  head_mid: number;
  head_bottom: number;
  robe: number;
  last_map: string;
  delete_date: number | null;
  sex: Gender;
}

interface CharacterListResponse {
  success: boolean;
  error?: string;
  characters?: CharacterData[];
}

interface CharacterSelectionProps {
  onCharacterSelected: () => void;
  onBackToServerSelection: () => void;
}

type Screen = 'loading' | 'list' | 'creation';

export default function CharacterSelection({
  onCharacterSelected,
  onBackToServerSelection
}: CharacterSelectionProps) {
  const { slotWithCharUrl, slotNoCharUrl } = useAssets();
  const [screen, setScreen] = useState<Screen>('loading');
  const [characters, setCharacters] = useState<CharacterData[]>([]);
  const [selectedSlot, setSelectedSlot] = useState<number | null>(null);
  const [creationSlot, setCreationSlot] = useState<number>(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const loadCharacters = async () => {
      try {
        const result = await invoke<CharacterListResponse>('get_character_list');

        if (result.success && result.characters) {
          setCharacters(result.characters);
          setScreen('list');
        } else {
          setError(result.error || 'Failed to load characters');
          setScreen('list');
        }
      } catch (err) {
        setError('Network error: ' + err);
        setScreen('list');
      }
    };

    loadCharacters();
  }, []);

  const handleCharacterSelect = async (slot: number, character: CharacterData | null) => {
    if (!character) {
      setCreationSlot(slot);
      setScreen('creation');
      return;
    }

    setSelectedSlot(slot);
  };

  const handlePlayCharacter = async () => {
    if (selectedSlot === null) return;

    console.log(`🎮 [FRONTEND] User clicked ENTER button for slot ${selectedSlot}`);
    setLoading(true);
    setError(null);

    try {
      console.log(`🎮 [FRONTEND] Invoking select_character command for slot ${selectedSlot}`);
      const result = await invoke<{ success: boolean; error?: string }>('select_character', {
        slot: selectedSlot
      });
      console.log(`🎮 [FRONTEND] Received response from select_character:`, result);

      if (result.success) {
        console.log(`🎮 [FRONTEND] Character selection successful, transitioning to in_game screen`);
        onCharacterSelected();
      } else {
        console.error(`🎮 [FRONTEND] Character selection failed:`, result.error);
        setError(result.error || 'Character selection failed');
      }
    } catch (err) {
      console.error(`🎮 [FRONTEND] Network error during character selection:`, err);
      setError('Network error: ' + err);
    } finally {
      setLoading(false);
    }
  };

  const handleCharacterCreated = async () => {
    try {
      const result = await invoke<CharacterListResponse>('get_character_list');

      if (result.success && result.characters) {
        setCharacters(result.characters);
      }
    } catch (err) {
      setError('Failed to reload character list');
    }

    setScreen('list');
  };

  const getJobClassName = (jobClass: JobClass): string => {
    const names: Record<JobClass, string> = {
      [JobClass.Novice]: 'Novice',
      [JobClass.Swordsman]: 'Swordsman',
      [JobClass.Magician]: 'Magician',
      [JobClass.Archer]: 'Archer',
      [JobClass.Acolyte]: 'Acolyte',
      [JobClass.Merchant]: 'Merchant',
      [JobClass.Thief]: 'Thief',
      [JobClass.Knight]: 'Knight',
      [JobClass.Priest]: 'Priest',
      [JobClass.Wizard]: 'Wizard',
      [JobClass.Blacksmith]: 'Blacksmith',
      [JobClass.Hunter]: 'Hunter',
      [JobClass.Assassin]: 'Assassin',
      [JobClass.Crusader]: 'Crusader',
      [JobClass.Monk]: 'Monk',
      [JobClass.Sage]: 'Sage',
      [JobClass.Rogue]: 'Rogue',
      [JobClass.Alchemist]: 'Alchemist',
      [JobClass.BardDancer]: 'Bard/Dancer',
    };
    return names[jobClass] || 'Unknown';
  };

  const getSlotBackgroundImage = (character: CharacterData | null): string | undefined => {
    // For now, we don't have blocked slot info, so we just use two states:
    // - Character exists: use slot_with_char
    // - Empty slot: use slot_no_char
    // TODO: Add blocked state when backend provides blocked slot info
    if (character) {
      return slotWithCharUrl || undefined;
    } else {
      return slotNoCharUrl || undefined;
    }
  };

  // Show simple loading indicator while characters load
  if (screen === 'loading') {
    return (
      <div className="character-selection-container">
        <div className="character-selection-box">
          <h1>Loading Characters...</h1>
        </div>
      </div>
    );
  }

  if (screen === 'creation') {
    return (
      <CharacterCreation
        selectedSlot={creationSlot}
        onCharacterCreated={handleCharacterCreated}
        onCancel={() => setScreen('list')}
      />
    );
  }

  return (
    <div
      className="character-selection-container"
      style={{
        background: 'transparent',
      }}
    >
      <div className="character-selection-box" style={{
        background: 'transparent',
        boxShadow: 'none',
      }}>
        <h1 style={{ display: 'none' }}>Select Character</h1>

        <div className="character-grid">
          {[...Array(8)].map((_, index) => {
            const character = characters[index] || null;
            const isSelected = selectedSlot === index;
            const backgroundImage = getSlotBackgroundImage(character);

            return (
              <div
                key={index}
                className={`character-card ${isSelected ? 'selected' : ''} ${!character ? 'empty' : ''}`}
                onClick={() => character && handleCharacterSelect(index, character)}
                style={{
                  backgroundImage: backgroundImage ? `url(${backgroundImage})` : undefined,
                  backgroundSize: 'contain',
                  backgroundPosition: 'center',
                  backgroundRepeat: 'no-repeat',
                }}
              >
                {character ? (
                  <>
                    <div className="character-sprite-container">
                      {/* Body sprite - no offset, this is the anchor */}
                      <SpriteImage
                        spritePath={getBodySpritePath(character.class, character.sex)}
                        actionIndex={0}
                        frameIndex={0}
                        scale={1.5}
                        className="character-body-sprite"
                        alt={`${character.name} body`}
                        applyOffset={false}
                      />

                      {/* Hair sprite with palette */}
                      <SpriteImage
                        spritePath={getHairSpritePath(character.hair_style, character.sex)}
                        actionIndex={0}
                        frameIndex={0}
                        palettePath={getHairPalettePath(
                          character.hair_style,
                          character.sex,
                          character.hair_color
                        )}
                        scale={1.5}
                        className="character-hair-sprite"
                        alt={`${character.name} hair`}
                      />
                    </div>
                    <div className="character-info">
                      <div className="character-name">{character.name}</div>
                      <div className="character-level">Lv. {character.base_level} / {character.job_level}</div>
                      <div className="character-class">{getJobClassName(character.class)}</div>
                    </div>
                  </>
                ) : (
                  <button
                    onClick={() => handleCharacterSelect(index, null)}
                    className="create-char-button"
                    disabled={loading}
                  >
                    Create Character
                  </button>
                )}
              </div>
            );
          })}
        </div>

        {error && <div className="error-message">{error}</div>}

        <div className="buttons-container">
          <button
            onClick={onBackToServerSelection}
            className="back-button"
            disabled={loading}
          >
            Back to Server Selection
          </button>

          <button
            onClick={handlePlayCharacter}
            disabled={loading || selectedSlot === null}
            className="play-button"
          >
            {loading ? 'Entering...' : 'Play'}
          </button>
        </div>
      </div>
    </div>
  );
}
