import { defineConfig } from 'vitepress'

// Project Pages uses /sms-pop-rs/. Override with WEBSITE_BASE=/ for a custom domain.
const base = process.env.WEBSITE_BASE || '/sms-pop-rs/'

export default defineConfig({
  lang: 'zh-CN',
  title: 'SmsPop',
  description: '将 iPhone 通知带到 Windows，验证码随手可用。',
  base,
  cleanUrls: false,
  lastUpdated: true,
  srcExclude: ['README.md'],
  head: [
    ['link', { rel: 'icon', type: 'image/svg+xml', href: `${base}app-icon.svg` }],
    ['meta', { name: 'theme-color', content: '#fafafa', media: '(prefers-color-scheme: light)' }],
    ['meta', { name: 'theme-color', content: '#191919', media: '(prefers-color-scheme: dark)' }],
  ],
  themeConfig: {
    logo: '/app-icon.svg',
    nav: [
      { text: '使用文档', link: '/guide/getting-started' },
      { text: '下载', link: 'https://github.com/fxaxg/sms-pop-rs/releases/latest' },
    ],
    socialLinks: [{ icon: 'github', link: 'https://github.com/fxaxg/sms-pop-rs' }],
    sidebar: [{
      text: '使用 SmsPop',
      items: [
        { text: '开始使用', link: '/guide/getting-started' },
        { text: '连接 iPhone', link: '/guide/pairing' },
        { text: '验证码填入', link: '/guide/otp' },
        { text: 'HTTP 网络接入', link: '/guide/network' },
        { text: '常见问题', link: '/guide/faq' },
        { text: '隐私说明', link: '/guide/privacy' },
      ],
    }, {
      text: '参与开发',
      items: [
        { text: 'Windows 更新与发版', link: '/development/releases' },
      ],
    }],
    outline: { label: '本页内容', level: [2, 3] },
    docFooter: { prev: '上一页', next: '下一页' },
    lastUpdated: { text: '最后更新' },
    sidebarMenuLabel: '目录',
    darkModeSwitchLabel: '切换外观',
    lightModeSwitchTitle: '切换为浅色模式',
    darkModeSwitchTitle: '切换为深色模式',
    returnToTopLabel: '返回顶部',
    footer: { message: '免费开源 · MIT License', copyright: 'SmsPop contributors' },
  },
})
