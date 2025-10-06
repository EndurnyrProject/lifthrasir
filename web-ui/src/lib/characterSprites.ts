/**
 * Character Sprite Path Utilities
 *
 * Provides helper functions to generate correct sprite paths for character rendering.
 * Handles Korean sprite names, gender suffixes, and palette paths for RO assets.
 */

/**
 * Character gender enum
 * Matches Rust Gender type on backend
 */
export enum Gender {
  Female = 0,
  Male = 1,
}

/**
 * Map JobClass enum values to Korean sprite names
 * These names correspond to the actual folder/file names in the RO data files
 */
const JOB_SPRITE_NAMES: Record<number, string> = {
  0: "초보자",      // Novice
  1: "검사",        // Swordsman
  2: "마법사",      // Magician
  3: "궁수",        // Archer
  4: "성직자",      // Acolyte
  5: "상인",        // Merchant
  6: "도둑",        // Thief
  7: "기사",        // Knight
  8: "프리스트",    // Priest
  9: "위저드",      // Wizard
  10: "제철공",     // Blacksmith
  11: "헌터",       // Hunter
  12: "어세신",     // Assassin
  14: "크루세이더", // Crusader
  15: "몽크",       // Monk
  16: "세이지",     // Sage
  17: "로그",       // Rogue
  18: "알케미스트", // Alchemist
  19: "바드댄서",   // Bard/Dancer
};

/**
 * Get gender suffix for sprite paths
 * 남 (nam) = male, 여 (yeo) = female
 */
function getGenderSuffix(gender: Gender): string {
  return gender === Gender.Female ? "여" : "남";
}

/**
 * Get body sprite path for a character
 * @param jobClass - Job class enum value (0-19)
 * @param gender - Character gender (Male/Female)
 * @returns Path to body sprite file
 */
export function getBodySpritePath(jobClass: number, gender: Gender): string {
  const sexSuffix = getGenderSuffix(gender);
  const jobSpriteName = JOB_SPRITE_NAMES[jobClass] || "초보자"; // Default to Novice

  return `data\\sprite\\인간족\\몸통\\${sexSuffix}\\${jobSpriteName}_${sexSuffix}.spr`;
}

/**
 * Get hair sprite path for a character
 * @param hairStyle - Hair style ID
 * @param gender - Character gender (Male/Female)
 * @returns Path to hair sprite file
 */
export function getHairSpritePath(hairStyle: number, gender: Gender): string {
  const sexSuffix = getGenderSuffix(gender);
  // Format: {style}_{gender}.spr (e.g., "1_여.spr")
  return `data\\sprite\\인간족\\머리통\\${sexSuffix}\\${hairStyle}_${sexSuffix}.spr`;
}

/**
 * Get hair palette path for a character
 * @param hairStyle - Hair style ID
 * @param gender - Character gender (Male/Female)
 * @param hairColor - Hair color ID (0 = no custom color)
 * @returns Path to hair palette file, or undefined if no custom color
 */
export function getHairPalettePath(
  hairStyle: number,
  gender: Gender,
  hairColor: number
): string | undefined {
  // Color 0 means default color (no palette needed)
  if (hairColor === 0) return undefined;

  const sexSuffix = getGenderSuffix(gender);
  // Format: {style}_{gender}_{color}.pal (e.g., "1_여_2.pal")
  return `data\\palette\\머리\\${hairStyle}_${sexSuffix}_${hairColor}.pal`;
}
