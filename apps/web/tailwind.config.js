/** @type {import('tailwindcss').Config} */
module.exports = {
  darkMode: 'class',
  content: [
    "./src/**/*.rs",
    "./index.html",
  ],
  theme: {
    extend: {
      colors: {
        gray: {
          750: '#2d3748',
        },
        theme: {
          base: 'var(--color-bg-base)',
          card: 'var(--color-bg-card)',
          elevated: 'var(--color-bg-elevated)',
          text: {
            primary: 'var(--color-text-primary)',
            secondary: 'var(--color-text-secondary)',
            muted: 'var(--color-text-muted)',
          },
          border: {
            default: 'var(--color-border-default)',
            hover: 'var(--color-border-hover)',
          },
          info: 'var(--color-info)',
          success: 'var(--color-success)',
          warning: 'var(--color-warning)',
          error: 'var(--color-error)',
        }
      },
    },
  },
  plugins: [],
}
