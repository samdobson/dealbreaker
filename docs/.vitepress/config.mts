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
      { text: 'Guide', link: '/getting-started' },
      { text: 'Games', link: '/games' },
      { text: 'Rules JSON', link: '/rules-format' },
      { text: 'Benchmarks', link: '/benchmarks' },
      { text: 'Live Demo', link: '/demo/', target: '_self' },
    ],

    sidebar: [
      {
        text: 'Start Here',
        items: [
          { text: 'Getting Started', link: '/getting-started' },
          { text: 'Built-in Games', link: '/games' },
          { text: 'How Answers Work', link: '/outcomes' },
        ],
      },
      {
        text: 'Using Dealbreaker',
        items: [
          { text: 'Command-Line (CLI)', link: '/cli' },
          { text: 'Libraries & Bindings', link: '/library' },
        ],
      },
      {
        text: 'Deep Dives',
        items: [
          { text: 'Custom Rules (JSON)', link: '/rules-format' },
          { text: 'Benchmarks', link: '/benchmarks' },
        ],
      },
    ],

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
