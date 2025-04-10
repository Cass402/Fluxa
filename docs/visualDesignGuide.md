# Visual Design Guide

## 1. Introduction

This document provides comprehensive visual design guidelines for Fluxa's user interface, with a specific focus on creating impactful visualizations for the hackathon demonstration. The guide emphasizes showcasing our core differentiating features: the concentrated liquidity AMM and impermanent loss (IL) mitigation system.

Our visual design philosophy centers on transforming complex DeFi mechanics into intuitive, visually accessible information that clearly demonstrates Fluxa's advantages. This approach helps both judges and future users understand the unique benefits of our platform.

## 2. Design System Fundamentals

### 2.1 Brand Identity

#### Color Palette

**Primary Colors:**

- **Fluxa Blue**: #3B82F6 (RGB: 59, 130, 246)
  - Used for primary buttons, key metrics, and branded elements
- **Fluxa Cyan**: #06B6D4 (RGB: 6, 182, 212)
  - Used for accents, highlights, and secondary elements

**Secondary Colors:**

- **Deep Purple**: #7C3AED (RGB: 124, 58, 237)
  - Used for gradients and feature highlights
- **Vibrant Coral**: #F43F5E (RGB: 244, 63, 94)
  - Used for alerts, warnings, and negative indicators

**Neutrals:**

- **Background Dark**: #111827 (RGB: 17, 24, 39)
- **Background Light**: #F8FAFC (RGB: 248, 250, 252)
- **Text Dark**: #1F2937 (RGB: 31, 41, 55)
- **Text Light**: #F9FAFB (RGB: 249, 250, 251)

**Data Visualization Colors:**

- **Positive**: #10B981 (RGB: 16, 185, 129)
- **Negative**: #EF4444 (RGB: 239, 68, 68)
- **Neutral**: #F59E0B (RGB: 245, 158, 11)
- **Comparison**: #8B5CF6 (RGB: 139, 92, 246)

#### Typography

**Primary Font: Inter**

- Headings: Inter Bold (700)
- Subheadings: Inter SemiBold (600)
- Body: Inter Regular (400)
- Data & Metrics: Inter Medium (500)

**Font Sizes:**

- H1: 32px (2rem)
- H2: 24px (1.5rem)
- H3: 20px (1.25rem)
- Body: 16px (1rem)
- Small/Caption: 14px (0.875rem)
- Data Labels: 12px (0.75rem)

**Line Heights:**

- Headings: 1.2
- Body: 1.5
- Data: 1.3

#### Iconography

Use a consistent icon set with the following characteristics:

- Rounded edges
- 2px stroke width
- 24x24 default size
- SVG format for scalability

### 2.2 Layout Principles

#### Grid System

- Base on 8px grid (0.5rem)
- 12-column layout for desktop
- 4-column layout for mobile
- Gutters: 16px (1rem)
- Margins: 24px (1.5rem) desktop, 16px (1rem) mobile

#### Spacing System

- 4px (0.25rem) - Minimal spacing (between related items)
- 8px (0.5rem) - Tight spacing
- 16px (1rem) - Standard spacing
- 24px (1.5rem) - Medium spacing
- 32px (2rem) - Large spacing
- 48px (3rem) - Section spacing

#### Component Sizing

- Button heights: 40px (2.5rem)
- Input heights: 40px (2.5rem)
- Card padding: 24px (1.5rem)
- Border radius: 8px (0.5rem)

### 2.3 Interactive Elements

#### Buttons

**Primary Button:**

- Background: Fluxa Blue
- Text: White
- Hover: 10% darker
- Active: 15% darker
- Border radius: 8px
- Padding: 8px 16px
- Height: 40px

**Secondary Button:**

- Border: 2px Fluxa Blue
- Text: Fluxa Blue
- Background: Transparent
- Hover: 10% Fluxa Blue background opacity
- Border radius: 8px

**Action Button:**

- Background: Gradient from Fluxa Blue to Deep Purple
- Text: White

#### Form Elements

**Text Input:**

- Border: 1px Neutral Gray
- Focus: 2px Fluxa Blue border
- Background: White (Light mode) / Dark Gray (Dark mode)
- Border radius: 8px
- Height: 40px
- Padding: 8px 12px

**Sliders:**

- Track: Light Gray
- Thumb: Fluxa Blue
- Active area: Fluxa Cyan
- Height: 6px
- Thumb size: 16px × 16px

## 3. Key Visualization Components

### 3.1 Liquidity Position Visualization

#### Concentrated Liquidity Range Display

The concentrated liquidity range display is a core visualization that shows:

1. **Price Range Selector:**

   - Current price marker (animated pulse)
   - Draggable handles for lower and upper bounds
   - Visible price labels at boundaries and current price
   - Shaded active range area

2. **Liquidity Distribution Graph:**

   - X-axis: Price range
   - Y-axis: Liquidity density
   - Curved area representation of liquidity concentration
   - User's position highlighted within overall pool liquidity

3. **Interactive Elements:**
   - Zoom controls for price range exploration
   - Hover tooltips showing exact values
   - "Reset to Current Price" button

**Mockup Description:**

```
┌─────────────────────────────────────────────────────────────┐
│             Liquidity Position                    [?]       │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                                                     │    │
│  │                    ╱╲                              │    │
│  │                   ╱  ╲                             │    │
│  │     ╱╲           ╱    ╲          ╱╲               │    │
│  │    ╱  ╲          │USER │         ╱  ╲              │    │
│  │___╱    ╲_________│POSN.│________╱    ╲_____________│    │
│  │                                                     │    │
│  └─────────────────────────────────────────────────────┘    │
│     $1.20    $1.30    $1.40     $1.50    $1.60             │
│              │                   │        │                 │
│          Lower Bound        Current    Upper Bound          │
│                                                             │
│    Drag handles to adjust position range                    │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 Impermanent Loss Visualization

#### IL Comparison Chart

The IL comparison chart is crucial for demonstrating Fluxa's advantage:

1. **Side-by-Side Comparison:**

   - Traditional AMM position (showing higher IL)
   - Concentrated liquidity position (showing medium IL)
   - Fluxa position with IL mitigation (showing reduced IL)

2. **Time-Series Animation:**

   - Show IL accumulation over time as price changes
   - Animated rebalancing of Fluxa position when volatility increases
   - Real-time adjustment of position boundaries

3. **Metrics Display:**
   - Percentage reduction in IL (prominently displayed)
   - Dollar value of saved impermanent loss
   - Comparison of ending position values

**Mockup Description:**

```
┌─────────────────────────────────────────────────────────────┐
│         Impermanent Loss Comparison                [?]      │
│                                                             │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐   │
│  │ Traditional   │  │ Concentrated  │  │ Fluxa with    │   │
│  │ AMM           │  │ Liquidity     │  │ IL Mitigation │   │
│  │               │  │               │  │               │   │
│  │   -5.7% IL    │  │   -4.2% IL    │  │   -2.9% IL    │   │
│  │               │  │               │  │               │   │
│  │ [Graph showing│  │ [Graph showing│  │ [Graph showing│   │
│  │  steep IL]    │  │  medium IL]   │  │  reduced IL]  │   │
│  │               │  │               │  │               │   │
│  │ Starting:$1000│  │ Starting:$1000│  │ Starting:$1000│   │
│  │ Current: $943 │  │ Current: $958 │  │ Current: $971 │   │
│  └───────────────┘  └───────────────┘  └───────────────┘   │
│                                                             │
│     IL Reduction: 49% vs Traditional, 31% vs Concentrated   │
└─────────────────────────────────────────────────────────────┘
```

### 3.3 Position Rebalancing Visualization

#### Rebalancing Animation

This visualization demonstrates how Fluxa's IL mitigation works:

1. **Before/After Display:**

   - Original position boundaries
   - Volatility indicator rising
   - Animated transition to new boundaries

2. **Boundary Adjustment Animation:**

   - Smooth animation of range expansion/contraction
   - Visual indication of rebalancing trigger points
   - Comparison with static position (no rebalancing)

3. **Performance Metrics:**
   - IL saved through rebalancing
   - Fee income maintained despite range changes
   - Net benefit calculation

**Mockup Description:**

```
┌─────────────────────────────────────────────────────────────┐
│         Dynamic Position Rebalancing                [?]     │
│                                                             │
│  Volatility: ■■■■■■■□□□ 70%    Rebalance Status: Active    │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                    PRICE CHART                      │    │
│  │                                                     │    │
│  │   ╭─────╮                                          │    │
│  │   │     │  ╭───────╮                               │    │
│  │   │     │  │       │    ╭───────────╮              │    │
│  │   │     ╰──╯       ╰────╯           ╰─────         │    │
│  │                                                     │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                             │
│  ┌───────────────────────┐  ┌───────────────────────┐      │
│  │ Original Boundaries   │  │ Adjusted Boundaries   │      │
│  │                       │  │                       │      │
│  │ Lower: $1.30          │  │ Lower: $1.25 (-3.8%)  │      │
│  │ Upper: $1.60          │  │ Upper: $1.70 (+6.3%)  │      │
│  │                       │  │                       │      │
│  │ Range Width: 23.1%    │  │ Range Width: 36.0%    │      │
│  └───────────────────────┘  └───────────────────────┘      │
│                                                             │
│  Benefit: -2.1% IL vs -3.5% IL without adjustment          │
└─────────────────────────────────────────────────────────────┘
```

### 3.4 Yield Strategy Visualization

#### Risk Profile Selector

This visualization demonstrates the simplified yield optimization component:

1. **Strategy Selector:**

   - Visual slider with three positions (Conservative, Balanced, Aggressive)
   - Illustrated risk vs. reward tradeoff
   - Profile characteristics summary

2. **Strategy Comparison:**

   - Side-by-side metrics for each strategy
   - Expected yield ranges
   - Rebalancing frequency
   - IL protection level

3. **Projected Performance:**
   - Simulated performance chart for selected strategy
   - Historical comparison data
   - Confidence intervals

**Mockup Description:**

```
┌─────────────────────────────────────────────────────────────┐
│         Personalized Yield Strategy                [?]      │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                                                     │    │
│  │  Conservative    ●      Balanced      ●   Aggressive│    │
│  │                                                     │    │
│  │  Lower Risk                              Higher Risk│    │
│  │  Lower Return                            Higher Return   │
│  └─────────────────────────────────────────────────────┘    │
│                                                             │
│  Selected: Balanced                                         │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ Strategy Characteristics:                           │    │
│  │ • Medium range width (30-50% around current price)  │    │
│  │ • Weekly rebalancing frequency                      │    │
│  │ • Target APR: 15-25%                                │    │
│  │ • IL Protection: Medium (60% effectiveness)         │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                             │
│  [Apply Strategy] [View Simulation]                         │
└─────────────────────────────────────────────────────────────┘
```

## 4. Dashboard Design

### 4.1 Main Dashboard Layout

The main dashboard is designed to provide a comprehensive overview while emphasizing our key differentiators:

```
┌─────────────────────────────────────────────────────────────┐
│ Logo  Pool Selection ▼              Connect Wallet  [≡]     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐   │
│  │ Pool Overview │  │ Your Position │  │ IL Protection │   │
│  │ SOL/USDC      │  │ $5,000        │  │ Status: Active│   │
│  │ $10M TVL      │  │ Range: ±25%   │  │ Saved: $120   │   │
│  └───────────────┘  └───────────────┘  └───────────────┘   │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                                                     │    │
│  │               POSITION VISUALIZATION                │    │
│  │                                                     │    │
│  │               (As detailed in 3.1)                  │    │
│  │                                                     │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                             │
│  ┌────────────────────────┐  ┌────────────────────────┐    │
│  │                        │  │                        │    │
│  │  IL COMPARISON         │  │  PERFORMANCE METRICS   │    │
│  │                        │  │                        │    │
│  │  (As detailed in 3.2)  │  │  • Earned: 120 USDC    │    │
│  │                        │  │  • APR: 18.5%          │    │
│  │                        │  │  • IL Saved: 2.3%      │    │
│  │                        │  │  • Next rebalance: 2d  │    │
│  │                        │  │                        │    │
│  └────────────────────────┘  └────────────────────────┘    │
│                                                             │
│  [Manage Position] [Adjust Strategy] [Add Liquidity]        │
└─────────────────────────────────────────────────────────────┘
```

### 4.2 Position Creation Flow

The position creation flow is designed to be intuitive while emphasizing Fluxa's advantages:

1. **Token Selection**

   - Pair selection with popular pairs highlighted
   - Balance display
   - Price and liquidity depth information

2. **Range Selection**

   - Interactive range selector (as in 3.1)
   - Preset ranges (Narrow, Medium, Wide)
   - Custom range input
   - Capital efficiency calculator

3. **Strategy Selection**

   - Risk profile selection (as in 3.4)
   - Fee tier selection
   - IL mitigation settings (Standard, Aggressive, Custom)

4. **Confirmation**
   - Position summary
   - Fee breakdown
   - Expected performance metrics
   - IL protection visualization

### 4.3 Analytics View

The analytics view provides detailed performance data and comparisons:

1. **Historical Performance**

   - Time-series chart of position value
   - Comparisons with benchmarks (HODL, Traditional AMM)
   - Fee income breakdown

2. **IL Analysis**

   - IL timeline
   - Rebalancing events overlay
   - Saved IL metrics

3. **What-If Scenarios**
   - Simulation tools for different market conditions
   - Strategy optimization recommendations
   - Manual override options

## 5. Animation and Interaction Guidelines

### 5.1 Animation Principles

All animations should follow these principles:

1. **Purpose**: Animations should communicate meaning, not just look appealing
2. **Subtlety**: Use subtle animations that don't distract from content
3. **Speed**: Aim for 200-300ms duration for most UI transitions
4. **Easing**: Use ease-out for entrances, ease-in for exits, ease-in-out for transitions

### 5.2 Key Animations

#### Price Movement Animation

- Smooth transitions between price points
- Subtle bouncing effect at significant movements
- Color-coding based on direction (green up, red down)

#### Range Adjustment Animation

- Elastic effect when dragging range boundaries
- Ripple effect when boundaries are set
- Fade transition when showing different range options

#### Rebalancing Visualization

- Slow-motion replay of position adjustments
- Side-by-side comparison with non-rebalancing position
- Progressive reveal of benefits gained

### 5.3 Interactive Elements

#### Tooltips and Information

- Clear, concise tooltips for all specialized terms
- Consistent positioning and styling
- Progressive information disclosure

#### Data Exploration

- Zoom and pan capabilities on charts
- Timeline scrubber for historical data
- Adjustable parameters with real-time updates

#### Simulation Controls

- Time scale control (speed up/slow down)
- Volatility adjustment sliders
- Scenario selection dropdown

## 6. Responsive Design Considerations

### 6.1 Viewport Adaptations

Design responsive adaptations for:

- Desktop (1920px and below)
- Tablet (1024px and below)
- Mobile (640px and below)

### 6.2 Component Adaptation Rules

- Stack cards vertically on smaller screens
- Simplify visualizations for mobile
- Preserve critical metrics at all viewport sizes
- Use collapsible sections for detailed information

### 6.3 Touch Interactions

Ensure all interactive elements are:

- Minimum 44×44 points touch target
- Clear visual feedback on touch
- Adequate spacing between touch targets

## 7. Accessibility Guidelines

### 7.1 Color and Contrast

- Maintain minimum 4.5:1 contrast ratio for text
- Don't rely solely on color to convey information
- Provide high contrast mode option

### 7.2 Text and Readability

- Minimum 16px font size for body text
- Ability to increase text size up to 200%
- Clear visual hierarchy through typography

### 7.3 Screen Reader Support

- Proper ARIA labels for all interactive elements
- Meaningful alt-text for informational graphics
- Logical tab order for navigation

## 8. Implementation Notes for Hackathon

### 8.1 Technical Constraints

- Use React for component development
- Leverage D3.js for data visualizations
- Ensure 60fps performance for animations
- Optimize for demo environment (Chrome latest)

### 8.2 Prioritization for Hackathon Demo

1. **Must Have Visualizations**:

   - Concentrated liquidity position display (3.1)
   - IL comparison chart (3.2)
   - Position rebalancing visualization (3.3)

2. **Secondary Visualizations**:

   - Yield strategy selector (3.4)
   - Analytics views

3. **Polish Elements** (if time allows):
   - Advanced animations
   - Additional what-if scenarios
   - Expanded data visualization options

### 8.3 Implementation Timeline

| Phase                  | Focus                           | Timeline   | Priority |
| ---------------------- | ------------------------------- | ---------- | -------- |
| Core Components        | Basic UI components and layout  | Days 1-7   | High     |
| Primary Visualizations | Position and IL displays        | Days 8-14  | High     |
| Interactive Features   | Range selection and adjustments | Days 15-21 | High     |
| Animation & Polish     | Smooth transitions and effects  | Days 22-28 | Medium   |

## 9. Design Resources and Assets

### 9.1 Component Library

A Figma component library is available at [Fluxa Design System](https://figma.com/team/fluxa/design-system) containing:

- Core UI components
- Visualization templates
- Icon library
- Color and typography styles

### 9.2 Data Visualization Templates

Pre-built chart and graph templates for:

- Price charts
- Liquidity distributions
- IL comparisons
- Performance metrics

### 9.3 Animation Library

A collection of reusable animations for:

- Position adjustments
- Price movements
- State transitions
- Loading states

## 10. Testing and Validation

### 10.1 Visual Testing Checklist

- [ ] Core visualizations render correctly at all viewport sizes
- [ ] Animations perform at target frame rate
- [ ] Color contrast meets accessibility standards
- [ ] Interactive elements have appropriate hover/focus states
- [ ] Dark mode compatibility

### 10.2 User Feedback Process

1. Internal team review at 50% completion
2. Structured user testing with 3-5 DeFi users
3. Refinement based on feedback
4. Final validation with demonstration rehearsal

### 10.3 Pre-Submission Review

Final review focused on:

- Visual impact of key differentiators
- Clarity of demonstration flow
- Performance optimization
- Fallback options for demo environment

## 11. Conclusion

This visual design guide provides a comprehensive framework for creating an impactful, intuitive interface for Fluxa's hackathon submission. By focusing on clear visualization of our core differentiators—concentrated liquidity and impermanent loss mitigation—we can create a compelling demonstration that showcases our unique value proposition.

The design principles, component specifications, and visualization requirements outlined in this document should be followed closely to ensure consistency and maximize impact. By prioritizing visual clarity, interactive exploration, and meaningful comparisons, we can effectively communicate Fluxa's advantages to hackathon judges and future users.
