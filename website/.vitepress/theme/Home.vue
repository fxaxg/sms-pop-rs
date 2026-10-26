<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useData, withBase } from 'vitepress'

const { isDark } = useData()
const copied = ref(false)
const copyFailed = ref(false)
let resetTimer: ReturnType<typeof setTimeout> | undefined
const themeLabel = computed(() => isDark.value ? '切换为浅色模式' : '切换为深色模式')

onMounted(() => {
  document.documentElement.classList.add('smspop-landing')
})
onBeforeUnmount(() => {
  document.documentElement.classList.remove('smspop-landing')
  if (resetTimer) clearTimeout(resetTimer)
})

function toggleTheme() {
  isDark.value = !isDark.value
}

async function copyDemo() {
  if (resetTimer) clearTimeout(resetTimer)
  try {
    await navigator.clipboard.writeText('482913')
    copied.value = true
    copyFailed.value = false
  } catch {
    copied.value = false
    copyFailed.value = true
  }
  resetTimer = setTimeout(() => {
    copied.value = false
    copyFailed.value = false
  }, 2400)
}
</script>

<template>
  <div class="landing">
    <a class="landing-skip" href="#main">跳转到内容</a>
    <div class="hero-art" aria-hidden="true">
      <div class="ribbon" :style="{
        '--ribbon-light': `url(${withBase('/ribbon-light.png')})`,
        '--ribbon-dark': `url(${withBase('/ribbon-dark.png')})`,
      }"></div>
    </div>

    <header class="site-header">
      <a class="site-brand" :href="withBase('/')" aria-label="SmsPop 首页">
        <img :src="withBase('/app-icon.svg')" width="38" height="38" alt="" />
        <span>SmsPop</span>
      </a>
      <nav class="site-nav" aria-label="主导航">
        <a :href="withBase('/guide/getting-started.html')">使用文档</a>
        <a class="github-link" href="https://github.com/fxaxg/sms-pop-rs" target="_blank" rel="noreferrer">GitHub<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M5 3h8v8M13 3 3 13" /></svg></a>
        <span class="nav-divider" aria-hidden="true"></span>
        <ClientOnly>
          <button class="theme-button" type="button" :aria-label="themeLabel" :title="themeLabel" @click="toggleTheme">
          <svg v-if="isDark" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="4" /><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5m11 11L19 19M5 19l1.5-1.5m11-11L19 5" /></svg>
          <svg v-else viewBox="0 0 24 24" aria-hidden="true"><path d="M20.5 13A8.5 8.5 0 0 1 11 3.5 8.5 8.5 0 1 0 20.5 13Z" /></svg>
          </button>
          <template #fallback><span class="theme-button" aria-hidden="true"></span></template>
        </ClientOnly>
      </nav>
    </header>

    <main id="main" class="hero">
      <div class="hero-copy">
        <h1>手机通知，<br />电脑上见。</h1>
        <p class="hero-description">将 iPhone 通知带到 Windows，<span>验证码随手可用。</span></p>
        <div class="hero-actions">
          <a class="download-button" href="https://github.com/fxaxg/sms-pop-rs/releases/latest">
            <svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M2 4.6 10.5 3.4v8H2Zm10-1.5L22 1.8v9.6H12ZM2 12.9h8.5v8L2 19.7Zm10 0h10v9.4L12 21Z" /></svg>
            下载 Windows 版
            <svg class="download-arrow" viewBox="0 0 24 24" aria-hidden="true"><path d="M12 4v12m-5-5 5 5 5-5M5 19v2h14v-2" /></svg>
          </a>
          <p class="download-note">免费开源<span aria-hidden="true"> · </span><span>适用于 Windows</span></p>
        </div>
      </div>

      <div class="notification-stage">
        <div class="notification-demo" role="group" aria-label="通知效果演示，验证码为示例内容">
          <div class="notification-meta">
            <img :src="withBase('/app-icon.svg')" width="27" height="27" alt="" />
            <span>信息<span class="meta-dot">·</span>刚刚</span>
            <span class="demo-label">演示</span>
          </div>
          <div class="notification-content">
            <p>验证码 <span class="sample-code">482913</span></p>
            <button class="copy-button" type="button" @click="copyDemo">{{ copied ? '已复制' : '复制' }}</button>
          </div>
          <span class="sr-only" role="status">{{ copied ? '示例验证码已复制' : copyFailed ? '复制失败，请手动选择示例验证码' : '' }}</span>
        </div>
      </div>
    </main>

    <footer class="site-footer">
      <span>SmsPop <span class="footer-dot">·</span> <a href="https://github.com/fxaxg/sms-pop-rs/blob/main/LICENSE">MIT License</a></span>
      <a :href="withBase('/guide/privacy.html')">隐私说明<svg viewBox="0 0 16 16" aria-hidden="true"><path d="m6 4 4 4-4 4" /></svg></a>
    </footer>
  </div>
</template>
