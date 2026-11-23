import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Dealbreaker',
  description: 'A fast patience (solitaire) solver written in Rust, where every game is data.',
  base: '/dealbreaker/',
  cleanUrls: true,
  lastUpdated: true,
  ignoreDeadLinks: [/^\/demo/],

  vite: {
    css: {
      postcss: {},
    },
  },

  head: [
    [
      'link',
      {
        rel: 'icon',
        type: 'image/svg+xml',
        href: 'data:image/svg+xml,<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><rect width="32" height="32" rx="7" fill="%230f3d2e"/><text x="16" y="23" font-size="20" text-anchor="middle" fill="%23f4b942">♠</text></svg>',
      },
    ],
  ],

  themeConfig: {
    siteTitle: 'Dealbreaker',

    nav: [
      { text: 'Tutorials', link: '/tutorials/', activeMatch: '/tutorials/' },
      { text: 'How-to', link: '/how-to/', activeMatch: '/how-to/' },
      { text: 'Reference', link: '/reference/', activeMatch: '/reference/' },
      { text: 'Explanation', link: '/explanation/', activeMatch: '/explanation/' },
      { text: 'Live Demo', link: '/demo/', target: '_self' },
    ],

    sidebar: {
      '/tutorials/': [
        {
          text: 'Tutorials',
          items: [
            { text: 'Overview', link: '/tutorials/' },
            { text: 'Your First Solve', link: '/tutorials/first-solve' },
            { text: 'Build a Custom Game', link: '/tutorials/custom-game' },
            { text: 'Add Fair Hints to a Web App', link: '/tutorials/web-app-hints' },
          ],
        },
      ],
      '/how-to/': [
        {
          text: 'How-to Guides',
          items: [
            { text: 'Overview', link: '/how-to/' },
            { text: 'CLI & Scripting', link: '/how-to/cli-and-scripting' },
            { text: 'Fair Hints & Cancellation', link: '/how-to/fair-hints' },
            { text: 'Solve Points Games', link: '/how-to/points-games' },
            { text: 'Tune Search & Budgets', link: '/how-to/tune-search' },
            { text: 'Custom Rust Hooks', link: '/how-to/custom-rust-hooks' },
            { text: 'Embed in C, Swift, or Kotlin', link: '/how-to/mobile-and-native' },
          ],
        },
      ],
      '/reference/': [
        {
          text: 'Reference (Auto-generated)',
          items: [
            { text: 'Overview', link: '/reference/' },
            { text: 'Built-in Presets (39)', link: '/reference/presets' },
            { text: 'Rules Schema (JSON)', link: '/reference/rules-schema' },
            { text: 'Data Formats', link: '/reference/data-formats' },
            { text: 'CLI Reference', link: '/reference/cli' },
            { text: 'JavaScript / TypeScript', link: '/reference/javascript' },
            { text: 'Python', link: '/reference/python' },
            { text: 'C ABI', link: '/reference/c-abi' },
            { text: 'Rust API (docs.rs)', link: 'https://docs.rs/dealbreaker' },
          ],
        },
      ],
      '/explanation/': [
        {
          text: 'Explanation',
          items: [
            { text: 'Overview', link: '/explanation/' },
            { text: 'Verdicts & Soundness', link: '/explanation/verdicts-and-soundness' },
            { text: 'Hidden Cards & Fair Hints', link: '/explanation/hidden-cards' },
            { text: 'Engine & Search Architecture', link: '/explanation/engine-and-search' },
            { text: 'Scoring & Branch-and-Bound', link: '/explanation/scoring-and-bounds' },
            { text: 'Benchmarks', link: '/explanation/benchmarks' },
            { text: 'Stability & SemVer', link: '/explanation/stability' },
          ],
        },
      ],
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/samdobson/dealbreaker' },
    ],

    search: {
      provider: 'local',
    },

    editLink: {
      pattern: 'https://github.com/samdobson/dealbreaker/edit/main/docs/:path',
      text: 'Edit this page on GitHub',
    },

    footer: {
      message: 'Dual-licensed under MIT or Apache-2.0.',
      copyright: 'Dealbreaker — every solitaire game is data.',
    },
  },
})
