/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    './src/pages/**/*.{js,ts,jsx,tsx,mdx}',
    './src/components/**/*.{js,ts,jsx,tsx,mdx}',
    './src/app/**/*.{js,ts,jsx,tsx,mdx}',
  ],
  theme: {
    extend: {
      backgroundImage: {
        'gradient-radial': 'radial-gradient(var(--tw-gradient-stops))',
        'gradient-conic':
          'conic-gradient(from 180deg at 50% 50%, var(--tw-gradient-stops))',
      },
      colors: {
        primary: {
          DEFAULT: '#3B82F6', // Fluxa Blue
          light: '#93C5FD',
          dark: '#1D4ED8',
        },
        secondary: {
          DEFAULT: '#06B6D4', // Fluxa Cyan
          light: '#67E8F9',
          dark: '#0E7490',
        },
        accent: {
          DEFAULT: '#7C3AED', // Deep Purple
          light: '#C4B5FD',
          dark: '#5B21B6',
        },
        critical: {
          DEFAULT: '#F43F5E', // Vibrant Coral
          light: '#FDA4AF',
          dark: '#BE123C',
        },
        success: {
          DEFAULT: '#10B981', // Positive
          light: '#6EE7B7',
          dark: '#047857',
        },
        warning: {
          DEFAULT: '#F59E0B', // Neutral
          light: '#FCD34D',
          dark: '#B45309',
        },
        background: {
          dark: '#111827',
          light: '#F8FAFC',
        },
        text: {
          dark: '#1F2937',
          light: '#F9FAFB',
        }
      },
      fontFamily: {
        sans: ['Inter', 'sans-serif'],
      },
      borderRadius: {
        DEFAULT: '0.5rem', // 8px
      },
      spacing: {
        minimal: '0.25rem', // 4px
        tight: '0.5rem',    // 8px
        standard: '1rem',   // 16px
        medium: '1.5rem',   // 24px
        large: '2rem',      // 32px
        section: '3rem',    // 48px
      },
      boxShadow: {
        'sm': '0 1px 3px rgba(0, 0, 0, 0.1)',
        DEFAULT: '0 4px 6px rgba(0, 0, 0, 0.05), 0 1px 3px rgba(0, 0, 0, 0.1)',
        'md': '0 10px 15px rgba(0, 0, 0, 0.05), 0 4px 6px rgba(0, 0, 0, 0.05)',
        'lg': '0 20px 25px rgba(0, 0, 0, 0.1), 0 10px 10px rgba(0, 0, 0, 0.04)',
      },
      animation: {
        'fade-in': 'fadeIn 250ms cubic-bezier(0.0, 0, 0.2, 1)',
        'fade-out': 'fadeOut 150ms cubic-bezier(0.4, 0, 1, 1)',
        'slide-in': 'slideIn 250ms cubic-bezier(0.4, 0, 0.2, 1)',
        'slide-out': 'slideOut 200ms cubic-bezier(0.4, 0, 1, 1)',
        'pulse-blue': 'pulseBlue 2s cubic-bezier(0.4, 0, 0.6, 1) infinite',
        'ping': 'ping 1.5s cubic-bezier(0, 0, 0.2, 1) infinite',
        'bounce': 'bounce 1s infinite',
        'pulse': 'pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite',
        'float': 'float 10s ease-in-out infinite',
        'gradient-shift': 'gradient-shift 15s ease infinite',
        'text-shine': 'text-shine 5s linear infinite',
        'dash': 'dash 15s linear infinite alternate',
      },
      keyframes: {
        fadeIn: {
          '0%': { opacity: 0 },
          '100%': { opacity: 1 },
        },
        fadeOut: {
          '0%': { opacity: 1 },
          '100%': { opacity: 0 },
        },
        slideIn: {
          '0%': { transform: 'translateY(10px)', opacity: 0 },
          '100%': { transform: 'translateY(0)', opacity: 1 },
        },
        slideOut: {
          '0%': { transform: 'translateY(0)', opacity: 1 },
          '100%': { transform: 'translateY(10px)', opacity: 0 },
        },
        pulseBlue: {
          '0%, 100%': { 
            backgroundColor: 'rgba(59, 130, 246, 0.5)',
            boxShadow: '0 0 0 0 rgba(59, 130, 246, 0.7)',
          },
          '50%': { 
            backgroundColor: 'rgba(59, 130, 246, 0.8)',
            boxShadow: '0 0 0 10px rgba(59, 130, 246, 0)',
          },
        },
        ping: {
          '75%, 100%': {
            transform: 'scale(2)',
            opacity: '0',
          },
        },
        bounce: {
          '0%, 100%': {
            transform: 'translateY(-25%)',
            animationTimingFunction: 'cubic-bezier(0.8,0,1,1)',
          },
          '50%': {
            transform: 'none',
            animationTimingFunction: 'cubic-bezier(0,0,0.2,1)',
          },
        },
        pulse: {
          '50%': {
            opacity: '.5',
          },
        },
        float: {
          '0%': {
            transform: 'translateY(0px) translateX(0px)',
            opacity: '0',
          },
          '50%': {
            opacity: '0.8',
          },
          '100%': {
            transform: 'translateY(-100px) translateX(20px)',
            opacity: '0',
          },
        },
        'gradient-shift': {
          '0%': {
            backgroundPosition: '0% 50%',
          },
          '50%': {
            backgroundPosition: '100% 50%',
          },
          '100%': {
            backgroundPosition: '0% 50%',
          },
        },
        'text-shine': {
          'to': {
            backgroundPosition: '200% center',
          },
        },
        'dash': {
          'to': {
            strokeDashoffset: '1000',
          },
        },
      },
    },
  },
  plugins: [],
}