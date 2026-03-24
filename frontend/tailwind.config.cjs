/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    './index.html',
    './src/**/*.{js,ts,jsx,tsx}',
  ],
  theme: {
    extend: {
      colors: {
        omni: {
          blue: '#1E40AF',      // Royal Blue (Omnimon armor)
          red: '#DC2626',       // Crimson Red (Omnimon cape)
          gold: '#F59E0B',      // Gold Accent (details)
          navy: '#0F172A',      // Navy Deep (background)
          white: '#F8FAFC',     // Omni White
          silver: '#E2E8F0',    // Silver
        },
      },
      fontFamily: {
        sans: ['Inter', 'sans-serif'],
        mono: ['JetBrains Mono', 'monospace'],
      },
      backgroundImage: {
        'gradient-hero': 'linear-gradient(135deg, #1E40AF 0%, #DC2626 100%)',
        'gradient-shine': 'linear-gradient(to right, #F8FAFC, #E2E8F0, #1E40AF)',
      },
      animation: {
        'pulse-slow': 'pulse 3s cubic-bezier(0.4, 0, 0.6, 1) infinite',
      },
    },
  },
  plugins: [],
}
