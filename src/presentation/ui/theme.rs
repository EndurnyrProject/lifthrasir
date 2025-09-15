use bevy::prelude::*;

// Ashen Forged Color Palette - Main colors
pub const FORGE_SOOT: Color = Color::srgb(0.102, 0.110, 0.125); // #1A1C20 - Primary background
pub const SLATE_GRAY: Color = Color::srgb(0.176, 0.188, 0.220); // #2D3038 - Secondary panels 
pub const POLISHED_STEEL: Color = Color::srgb(0.310, 0.329, 0.376); // #4F5460 - Hover/borders
pub const RUNIC_GLOW: Color = Color::srgb(0.431, 0.890, 0.957); // #6EE3F4 - Interactive accent
pub const ASHEN_WHITE: Color = Color::srgb(0.882, 0.882, 0.882); // #E1E1E1 - Primary text
pub const GILDED_ACCENT: Color = Color::srgb(0.831, 0.686, 0.216); // #D4AF37 - Gold titles

// Feedback Colors
pub const MUTED_JADE: Color = Color::srgb(0.243, 0.541, 0.420); // #3E8A6B - Success
pub const AMBER: Color = Color::srgb(0.780, 0.533, 0.235); // #C7883C - Warning  
pub const WORN_CRIMSON: Color = Color::srgb(0.643, 0.259, 0.259); // #A44242 - Error

// Semantic Color Aliases
pub const BACKGROUND_PRIMARY: Color = FORGE_SOOT;
pub const BACKGROUND_SECONDARY: Color = SLATE_GRAY;
pub const BORDER_COLOR: Color = POLISHED_STEEL;
pub const TEXT_PRIMARY: Color = ASHEN_WHITE;
pub const TEXT_SECONDARY: Color = Color::srgb(0.600, 0.600, 0.600); // Muted text
pub const TEXT_GOLD: Color = GILDED_ACCENT;
pub const BUTTON_NORMAL: Color = POLISHED_STEEL;
pub const BUTTON_HOVER: Color = RUNIC_GLOW;
pub const BUTTON_PRESSED: Color = Color::srgb(0.250, 0.260, 0.300); // Darker steel
pub const INPUT_BACKGROUND: Color = Color::srgb(0.220, 0.235, 0.260); // Darker than slate
pub const INPUT_BORDER: Color = POLISHED_STEEL;
pub const INPUT_BORDER_FOCUS: Color = RUNIC_GLOW;
pub const SUCCESS_COLOR: Color = MUTED_JADE;
pub const WARNING_COLOR: Color = AMBER;
pub const ERROR_COLOR: Color = WORN_CRIMSON;

// Enhanced Color Variants with Transparency
pub const BACKGROUND_PRIMARY_TRANSPARENT: Color =
    Color::srgba(0.102, 0.110, 0.125, TRANSPARENCY_STRONG);
pub const BACKGROUND_SECONDARY_TRANSPARENT: Color =
    Color::srgba(0.176, 0.188, 0.220, TRANSPARENCY_MODERATE);
pub const INPUT_BACKGROUND_TRANSPARENT: Color =
    Color::srgba(0.220, 0.235, 0.260, TRANSPARENCY_MODERATE);
pub const BUTTON_NORMAL_TRANSPARENT: Color = Color::srgba(0.310, 0.329, 0.376, TRANSPARENCY_SUBTLE);
pub const BUTTON_HOVER_TRANSPARENT: Color = Color::srgba(0.431, 0.890, 0.957, TRANSPARENCY_HOVER);
pub const BUTTON_PRESSED_TRANSPARENT: Color =
    Color::srgba(0.250, 0.260, 0.300, TRANSPARENCY_SUBTLE);

// Configurable Panel Background Colors
pub const PANEL_BACKGROUND_SUBTLE: Color =
    Color::srgba(0.176, 0.188, 0.220, PANEL_TRANSPARENCY_SUBTLE);
pub const PANEL_BACKGROUND_LIGHT: Color =
    Color::srgba(0.176, 0.188, 0.220, PANEL_TRANSPARENCY_LIGHT);
pub const PANEL_BACKGROUND_MEDIUM: Color =
    Color::srgba(0.176, 0.188, 0.220, PANEL_TRANSPARENCY_MEDIUM);
pub const PANEL_BACKGROUND_STRONG: Color =
    Color::srgba(0.176, 0.188, 0.220, PANEL_TRANSPARENCY_STRONG);
pub const PANEL_BACKGROUND_VERY_STRONG: Color =
    Color::srgba(0.176, 0.188, 0.220, PANEL_TRANSPARENCY_VERY_STRONG);

// Spacing System
pub const SPACING_XS: f32 = 4.0; // Tiny gaps
pub const SPACING_SM: f32 = 8.0; // Small gaps  
pub const SPACING_MD: f32 = 12.0; // Medium gaps
pub const SPACING_LG: f32 = 16.0; // Large gaps
pub const SPACING_XL: f32 = 24.0; // Extra large gaps
pub const SPACING_XXL: f32 = 32.0; // Section spacing

// Semantic Spacing Aliases
pub const WINDOW_PADDING: f32 = SPACING_XL;
pub const ELEMENT_SPACING: f32 = SPACING_MD;
pub const INPUT_PADDING: f32 = SPACING_SM;
pub const BORDER_WIDTH: f32 = 1.0;

// Transparency Levels (game-appropriate alpha values)
pub const TRANSPARENCY_SUBTLE: f32 = 0.95; // Barely visible, for depth
pub const TRANSPARENCY_MODERATE: f32 = 0.85; // Input backgrounds
pub const TRANSPARENCY_STRONG: f32 = 0.75; // Floating panels
pub const TRANSPARENCY_HOVER: f32 = 0.90; // Hover state accent

// Border Radius System (medieval/fantasy appropriate)
pub const RADIUS_SM: f32 = 4.0; // Small elements, inputs
pub const RADIUS_MD: f32 = 8.0; // Buttons, standard components
pub const RADIUS_LG: f32 = 12.0; // Panels, containers
pub const RADIUS_PILL: f32 = 999.0; // Pill-shaped buttons

// Panel Size Presets
pub const PANEL_SIZE_SMALL: (f32, f32) = (280.0, 200.0); // Compact login panel
pub const PANEL_SIZE_MEDIUM: (f32, f32) = (400.0, 300.0); // Standard dialog
pub const PANEL_SIZE_LARGE: (f32, f32) = (500.0, 400.0); // Large content panel
pub const PANEL_SIZE_FULL: (f32, f32) = (400.0, 500.0); // Original full panel

// Panel Transparency Levels
pub const PANEL_TRANSPARENCY_SUBTLE: f32 = 0.95; // Barely transparent
pub const PANEL_TRANSPARENCY_LIGHT: f32 = 0.85; // Light transparency
pub const PANEL_TRANSPARENCY_MEDIUM: f32 = 0.75; // Medium transparency
pub const PANEL_TRANSPARENCY_STRONG: f32 = 0.65; // Strong transparency
pub const PANEL_TRANSPARENCY_VERY_STRONG: f32 = 0.55; // Very transparent

// Component Dimensions (Compact sizes to match smaller fonts)
pub const BUTTON_HEIGHT: f32 = 32.0; // Reduced from 40.0
pub const INPUT_HEIGHT: f32 = 28.0; // Reduced from 35.0
pub const CHECKBOX_SIZE: f32 = 16.0; // Reduced from 20.0

// Typography Scale (Compact sizes for smaller panels)
pub const FONT_SIZE_TITLE: f32 = 24.0; // Reduced from 32.0
pub const FONT_SIZE_SUBTITLE: f32 = 12.0; // Reduced from 16.0
pub const FONT_SIZE_LABEL: f32 = 14.0; // Reduced from 18.0
pub const FONT_SIZE_BODY: f32 = 12.0; // Reduced from 16.0
pub const FONT_SIZE_BUTTON: f32 = 14.0; // Reduced from 18.0
pub const FONT_SIZE_SMALL: f32 = 10.0; // Small text for secondary info

// Component markers for theming
#[derive(Component)]
pub struct RoButton;

#[derive(Component)]
pub struct RoInput;

#[derive(Component)]
pub struct RoPanel;

#[derive(Component)]
pub struct RoText;
