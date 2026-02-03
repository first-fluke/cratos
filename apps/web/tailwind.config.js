/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./src/**/*.rs",
    "./index.html",
  ],
  theme: {
    extend: {
      colors: {
        gray: {
          750: 'rgb(45, 55, 72)',
        },
      },
    },
  },
  plugins: [],
}
