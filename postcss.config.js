//INFO: PostCSS pipeline for Tailwind v3 + autoprefixer (keeps the safari13
//      build target safe for AppImage distribution on older WebKitGTK).
export default {
  plugins: {
    tailwindcss: {},
    autoprefixer: {},
  },
};
