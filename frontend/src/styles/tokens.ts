/**
 * Fluxa Design System - Design Tokens
 * Based on the Visual Design Guide document
 */

export const colors = {
  // Primary Colors
  fluxaBlue: "#3B82F6", // RGB: 59, 130, 246
  fluxaCyan: "#06B6D4", // RGB: 6, 182, 212

  // Secondary Colors
  deepPurple: "#7C3AED", // RGB: 124, 58, 237
  vibrantCoral: "#F43F5E", // RGB: 244, 63, 94

  // Neutrals
  backgroundDark: "#111827", // RGB: 17, 24, 39
  backgroundLight: "#F8FAFC", // RGB: 248, 250, 252
  textDark: "#1F2937", // RGB: 31, 41, 55
  textLight: "#F9FAFB", // RGB: 249, 250, 251

  // Data Visualization Colors
  positive: "#10B981", // RGB: 16, 185, 129
  negative: "#EF4444", // RGB: 239, 68, 68
  neutral: "#F59E0B", // RGB: 245, 158, 11
  comparison: "#8B5CF6", // RGB: 139, 92, 246
};

export const typography = {
  // Font Family
  fontFamily: "Inter, sans-serif",

  // Font Weights
  fontWeights: {
    regular: 400,
    medium: 500,
    semibold: 600,
    bold: 700,
  },

  // Font Sizes
  fontSizes: {
    h1: "2rem", // 32px
    h2: "1.5rem", // 24px
    h3: "1.25rem", // 20px
    body: "1rem", // 16px
    small: "0.875rem", // 14px
    dataLabel: "0.75rem", // 12px
  },

  // Line Heights
  lineHeights: {
    headings: 1.2,
    body: 1.5,
    data: 1.3,
  },
};

export const spacing = {
  // Based on 8px grid system
  minimal: "0.25rem", // 4px - Minimal spacing (between related items)
  tight: "0.5rem", // 8px - Tight spacing
  standard: "1rem", // 16px - Standard spacing
  medium: "1.5rem", // 24px - Medium spacing
  large: "2rem", // 32px - Large spacing
  section: "3rem", // 48px - Section spacing
};

export const layout = {
  // Border Radius
  borderRadius: "0.5rem", // 8px

  // Component Sizing
  buttonHeight: "2.5rem", // 40px
  inputHeight: "2.5rem", // 40px
  cardPadding: "1.5rem", // 24px
};

export const elevation = {
  // Shadows
  shadow1: "0 1px 3px rgba(0, 0, 0, 0.1)",
  shadow2: "0 4px 6px rgba(0, 0, 0, 0.05), 0 1px 3px rgba(0, 0, 0, 0.1)",
  shadow3: "0 10px 15px rgba(0, 0, 0, 0.05), 0 4px 6px rgba(0, 0, 0, 0.05)",
  shadow4: "0 20px 25px rgba(0, 0, 0, 0.1), 0 10px 10px rgba(0, 0, 0, 0.04)",
};

export const animation = {
  // Timing
  duration: {
    fast: "150ms",
    normal: "250ms",
    slow: "350ms",
  },

  // Easing Functions
  easing: {
    easeOut: "cubic-bezier(0.0, 0, 0.2, 1)",
    easeIn: "cubic-bezier(0.4, 0, 1, 1)",
    easeInOut: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
};

// Define the tokens object before exporting it
const tokens = {
  colors,
  typography,
  spacing,
  layout,
  elevation,
  animation,
};

export default tokens;
