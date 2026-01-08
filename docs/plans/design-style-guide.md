# WiFi-Verify: Design Style Guide

## Overview

This document defines the visual design language for WiFi-Verify and its authentication system (Project Raindrops), ensuring consistency across all user interfaces. The design prioritizes clarity, professionalism, and ease of use while reflecting the dual-stack (IPv4/IPv6) nature of the product.

---

## Design Philosophy

### Principles

1. **Light and Clean**: White/light gray backgrounds reduce eye strain during extended use
2. **Functional First**: Design serves the data; visualizations are clear and readable
3. **Dual-Stack Identity**: Blue (IPv4) and orange (IPv6) are signature colors
4. **Professional but Approachable**: Technical tool that doesn't feel intimidating
5. **Consistency**: Same patterns across login, dashboard, network test, and survey pages

---

## Branding

### Product Names

- **WiFi-Verify**: The network measurement platform (nettest.html, dashboard, surveys)
- **Project Raindrops**: The authentication system (login page, access denied page)

Both share the same visual design language but use their respective branding in titles and footers.

---

## Color Palette

### Primary Colors

| Color | Hex | RGB | Usage |
|-------|-----|-----|-------|
| **IPv4 Blue** | `#2196F3` | rgb(33, 150, 243) | IPv4 data, primary actions, links |
| **IPv6 Orange** | `#FF9800` | rgb(255, 152, 0) | IPv6 data, secondary highlights |
| **Success Green** | `#4CAF50` | rgb(76, 175, 80) | Success states, positive metrics |
| **Error Red** | `#f44336` | rgb(244, 67, 54) | Errors, stop buttons, warnings |

### Neutral Colors

| Color | Hex | Usage |
|-------|-----|-------|
| **Text Primary** | `#333333` | Headings, important text |
| **Text Secondary** | `#666666` | Body text, descriptions |
| **Text Muted** | `#999999` | Hints, timestamps, footer |
| **Border** | `#dddddd` | Table borders, dividers |
| **Background Light** | `#f5f5f5` | Page background |
| **Background Card** | `#ffffff` | Card/container background |
| **Background Subtle** | `#fafafa` | Code blocks, info boxes |

### Extended Palette

| Color | Hex | Usage |
|-------|-----|-------|
| **IPv4 Blue Light** | `rgba(33, 150, 243, 0.1)` | IPv4 badge background |
| **IPv6 Orange Light** | `rgba(255, 152, 0, 0.1)` | IPv6 badge background |
| **IPv4 Blue Hover** | `#1976D2` | Button hover states |
| **IPv6 Orange Dark** | `#e65100` | Warning text |
| **Error Background** | `#ffebee` | Error message background |
| **Error Border** | `#ef9a9a` | Error message border |

---

## Typography

### Font Family

```css
font-family: Arial, sans-serif;
```

Arial is chosen for:
- Universal availability across all platforms
- Excellent readability at small sizes
- Clean, professional appearance
- Consistent rendering in browsers

### Font Sizes

| Element | Size | Weight |
|---------|------|--------|
| Page Title (h1) | 24px | bold |
| Section Title (h2) | 18px | bold |
| Subsection Title (h3) | 15px | bold |
| Body Text | 14px | normal |
| Button Text | 15px | 600 |
| Small/Caption | 12px | normal |
| Tiny (badges) | 11px | 500 |
| Monospace Data | 12px | normal |

### Font Colors

- **Headings**: `#333333`
- **Body**: `#666666`
- **Muted**: `#999999`
- **Links**: `#2196F3`
- **Error**: `#c62828`

---

## Spacing

### Base Unit

8px grid system. All spacing should be multiples of 8px.

### Common Spacing

| Name | Value | Usage |
|------|-------|-------|
| xs | 4px | Tight spacing, inline elements |
| sm | 8px | Between related elements |
| md | 16px | Standard padding, margins |
| lg | 24px | Section spacing |
| xl | 32px | Major section breaks |
| xxl | 40px | Container padding |

### Container Padding

- **Cards**: 40px
- **Sections**: 20px
- **Table Cells**: 8px
- **Buttons**: 12px vertical, 20px horizontal
- **Inputs**: 12px vertical, 14px horizontal

---

## Components

### Cards/Containers

```css
.card {
    background: white;
    border-radius: 8px;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.1);
    padding: 40px;
}

/* With accent border (login page) */
.card-accent {
    border-top: 4px solid #2196F3;
}

/* Warning/error accent */
.card-warning {
    border-top: 4px solid #FF9800;
}
```

### Buttons

```css
.btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 12px 20px;
    border: none;
    border-radius: 4px;
    font-size: 15px;
    font-weight: 600;
    cursor: pointer;
    transition: background-color 0.2s ease, box-shadow 0.2s ease;
}

.btn:hover {
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
}

/* Primary - IPv4 Blue */
.btn-primary {
    background-color: #2196F3;
    color: white;
}

.btn-primary:hover {
    background-color: #1976D2;
}

/* Danger/Stop */
.btn-danger {
    background-color: #f44336;
    color: white;
}

.btn-danger:hover {
    background-color: #d32f2f;
}

/* Disabled */
.btn:disabled {
    background-color: #ccc;
    color: #666;
    cursor: not-allowed;
    opacity: 0.6;
}
```

### Inputs

```css
input[type="text"],
input[type="password"] {
    width: 100%;
    padding: 12px 14px;
    border: 1px solid #ddd;
    border-radius: 4px;
    font-size: 15px;
    transition: border-color 0.2s ease, box-shadow 0.2s ease;
}

input:focus {
    border-color: #2196F3;
    box-shadow: 0 0 0 3px rgba(33, 150, 243, 0.1);
    outline: none;
}
```

### Tables

```css
table {
    border-collapse: collapse;
    width: 100%;
}

th, td {
    border: 1px solid #ddd;
    padding: 8px;
    text-align: center;
}

/* IPv4 header */
.ipv4 th {
    background-color: #2196F3;
    color: white;
}

/* IPv6 header */
.ipv6 th {
    background-color: #FF9800;
    color: white;
}

td:first-child {
    text-align: left;
}
```

### Badges

```css
.badge {
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 3px;
    font-weight: 500;
}

.badge-ipv4 {
    background-color: rgba(33, 150, 243, 0.1);
    color: #2196F3;
    border: 1px solid rgba(33, 150, 243, 0.3);
}

.badge-ipv6 {
    background-color: rgba(255, 152, 0, 0.1);
    color: #FF9800;
    border: 1px solid rgba(255, 152, 0, 0.3);
}
```

### Alerts/Messages

```css
.alert {
    padding: 12px;
    border-radius: 4px;
    font-size: 14px;
}

.alert-error {
    color: #c62828;
    background-color: #ffebee;
    border: 1px solid #ef9a9a;
}

.alert-success {
    color: #2e7d32;
    background-color: #e8f5e9;
    border: 1px solid #a5d6a7;
}

.alert-info {
    color: #1565c0;
    background-color: #e3f2fd;
    border: 1px solid #90caf9;
}

.alert-warning {
    color: #e65100;
    background-color: #fff3e0;
    border: 1px solid #ffcc80;
}
```

---

## Layout Patterns

### Page Structure

```
┌──────────────────────────────────────┐
│           Page Background            │
│           (#f5f5f5)                  │
│                                      │
│   ┌──────────────────────────────┐   │
│   │        Card Container        │   │
│   │        (white, shadow)       │   │
│   │                              │   │
│   │   ┌──────────────────────┐   │   │
│   │   │      Header          │   │   │
│   │   │   (logo, title)      │   │   │
│   │   └──────────────────────┘   │   │
│   │                              │   │
│   │   ┌──────────────────────┐   │   │
│   │   │      Content         │   │   │
│   │   │                      │   │   │
│   │   └──────────────────────┘   │   │
│   │                              │   │
│   │   ┌──────────────────────┐   │   │
│   │   │      Footer          │   │   │
│   │   └──────────────────────┘   │   │
│   └──────────────────────────────┘   │
│                                      │
└──────────────────────────────────────┘
```

### Dual-Stack Layout

For IPv4/IPv6 side-by-side comparison:

```css
.dual-stack-container {
    display: flex;
    gap: 20px;
}

.stack-column {
    flex: 1;
}
```

```
┌─────────────────┐  ┌─────────────────┐
│   IPv4 Data     │  │   IPv6 Data     │
│   (blue header) │  │ (orange header) │
│                 │  │                 │
└─────────────────┘  └─────────────────┘
```

---

## Icons

### Approach

- Use inline SVGs for consistency and color control
- Avoid emojis in production UI
- Icons should be single-color and match text or accent colors

### Common Icons

Icons are embedded as inline SVG for color control:

```html
<!-- Network/WiFi icon -->
<svg viewBox="0 0 24 24">
    <path d="M12 3C7.03 3 3 7.03 3 12s4.03 9 9 9 9-4.03 9-9-4.03-9-9-9zm0 16c-3.86 0-7-3.14-7-7s3.14-7 7-7 7 3.14 7 7-3.14 7-7 7z"/>
    <path d="M12 7c-2.76 0-5 2.24-5 5s2.24 5 5 5 5-2.24 5-5-2.24-5-5-5zm0 8c-1.65 0-3-1.35-3-3s1.35-3 3-3 3 1.35 3 3-1.35 3-3 3z"/>
    <circle cx="12" cy="12" r="1.5"/>
</svg>

<!-- Access denied/block icon -->
<svg viewBox="0 0 24 24">
    <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.42 0-8-3.58-8-8 0-1.85.63-3.55 1.69-4.9L16.9 18.31C15.55 19.37 13.85 20 12 20zm6.31-3.1L7.1 5.69C8.45 4.63 10.15 4 12 4c4.42 0 8 3.58 8 8 0 1.85-.63 3.55-1.69 4.9z"/>
</svg>
```

---

## Logo

### Primary Logo

The WiFi-Verify logo combines:
1. A gradient icon box (blue to orange)
2. Concentric circles suggesting network/signal
3. The "WiFi-Verify" wordmark

```html
<div class="logo">
    <div class="logo-icon">
        <!-- Gradient background: #2196F3 → #FF9800 -->
        <svg><!-- Network icon --></svg>
    </div>
    <h1>WiFi-Verify</h1>
</div>
```

### Logo Icon Styles

```css
.logo-icon {
    width: 40px;
    height: 40px;
    background: linear-gradient(135deg, #2196F3 0%, #FF9800 100%);
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
}

.logo-icon svg {
    width: 24px;
    height: 24px;
    fill: white;
}
```

---

## Responsive Design

### Breakpoints

| Name | Width | Usage |
|------|-------|-------|
| Mobile | < 480px | Single column, stacked elements |
| Tablet | 480px - 768px | Adjusted spacing |
| Desktop | > 768px | Full layout, side-by-side |

### Mobile Considerations

- Dual-stack layouts stack vertically on mobile
- Tables may need horizontal scroll
- Touch targets minimum 44x44px
- Reduced padding on small screens

---

## Animation

### Principles

- **Subtle**: Animations should not distract from data
- **Fast**: 200ms for most transitions
- **Purposeful**: Only animate meaningful state changes

### Standard Transitions

```css
/* Buttons, inputs */
transition: background-color 0.2s ease, box-shadow 0.2s ease;

/* Focus rings */
transition: border-color 0.2s ease, box-shadow 0.2s ease;

/* Hover effects */
transition: transform 0.2s ease;
```

### What NOT to Animate

- Large data updates (charts update without animation for performance)
- Metric values (number changes are instant)
- Page loads

---

## Accessibility

### Color Contrast

All text meets WCAG 2.1 AA standards:
- Normal text: 4.5:1 minimum
- Large text: 3:1 minimum

### Focus States

All interactive elements have visible focus states:

```css
input:focus,
button:focus,
a:focus {
    outline: none;
    box-shadow: 0 0 0 3px rgba(33, 150, 243, 0.3);
}
```

### Semantic HTML

- Use proper heading hierarchy (h1 → h2 → h3)
- Form inputs have associated labels
- Tables have proper headers
- Buttons use `<button>`, links use `<a>`

---

## Dark Mode (Future)

When implementing dark mode:

| Light Mode | Dark Mode |
|------------|-----------|
| `#f5f5f5` (background) | `#121212` |
| `#ffffff` (card) | `#1e1e1e` |
| `#333333` (text) | `#e0e0e0` |
| `#666666` (secondary) | `#a0a0a0` |
| `#ddd` (border) | `#333333` |

Primary colors (blue/orange) remain the same but may need slight saturation adjustments for dark backgrounds.

---

## File Organization

```
static/
├── css/
│   └── style.css        # (future) Shared styles
├── lib/
│   ├── chart.umd.js
│   └── ...
└── *.html               # Page templates

auth/src/
└── views.rs             # Embedded HTML/CSS for auth pages
```

---

## Examples

### Login Page (Project Raindrops)

- Light gray background (`#f5f5f5`)
- White card with blue top border
- Logo with gradient icon
- IPv4/IPv6 badges in header
- Blue primary buttons
- "Powered by Project Raindrops Authentication" in footer

### Network Test Page (WiFi-Verify)

- White background
- Blue IPv4 tables, orange IPv6 tables
- Dual-stack side-by-side layout
- Chart.js graphs with matching colors
- Functional, data-dense layout

### Access Denied Page (Project Raindrops)

- Light gray background
- White card with orange top border (warning)
- Clear error messaging
- Blue action button (logout)
- "Project Raindrops" in footer

---

## Summary

The WiFi-Verify design language is:

- **Light**: White and light gray backgrounds
- **Clean**: Simple borders, subtle shadows
- **Colored**: Blue for IPv4/primary, orange for IPv6/secondary
- **Functional**: Data-first, readable typography
- **Consistent**: Same patterns across all pages
