<script setup lang="ts">
import { openUrl } from '@tauri-apps/plugin-opener'
import { ChevronLeft } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'

const { t } = useI18n()
const router = useRouter()

const REPO_URL = 'https://github.com/LoosePrince/simple-todo'
const AUTHOR_URL = 'https://github.com/LoosePrince'

async function openUrlFallback(url: string) {
  try {
    await openUrl(url)
  } catch {
    window.open(url, '_blank', 'noopener')
  }
}

function openRepo() {
  openUrlFallback(REPO_URL)
}

function openAuthor() {
  openUrlFallback(AUTHOR_URL)
}

/** 快捷键列表：keys 为多组组合键，每组内用 + 连接，组与组之间为“或” */
const shortcutItems: { keys: string[][]; descKey: string }[] = [
  { keys: [['Enter']], descKey: 'about.shortcutAddTodo' },
  { keys: [['Ctrl', 'S']], descKey: 'about.shortcutSave' },
  { keys: [['Ctrl', 'Z']], descKey: 'about.shortcutUndo' },
  { keys: [['Ctrl', 'Y'], ['Ctrl', 'Shift', 'Z']], descKey: 'about.shortcutRedo' },
  { keys: [['Ctrl', 'B']], descKey: 'about.shortcutBold' },
  { keys: [['Ctrl', 'I']], descKey: 'about.shortcutItalic' }
]
</script>

<template>
  <div class="about-view">
    <div class="header">
      <h2>{{ t('about.title') }}</h2>
      <el-button @click="router.back()">
        <ChevronLeft :size="16" style="margin-right: 4px" />
        {{ t('common.back') }}
      </el-button>
    </div>

    <div class="about-content">
      <section class="about-section">
        <h3>{{ t('about.introTitle') }}</h3>
        <p class="about-text">{{ t('about.intro') }}</p>
      </section>

      <section class="about-section shortcuts-section">
        <h3>{{ t('about.shortcutsTitle') }}</h3>
        <ul class="shortcut-list">
          <li v-for="(item, index) in shortcutItems" :key="index" class="shortcut-row">
            <span class="shortcut-keys">
              <template v-for="(group, gi) in item.keys" :key="gi">
                <span v-if="gi > 0" class="shortcut-or">{{ t('about.shortcutOr') }}</span>
                <span class="kbd-group">
                  <template v-for="(key, ki) in group" :key="key">
                    <span v-if="ki > 0" class="kbd-plus">+</span>
                    <kbd class="kbd">{{ key }}</kbd>
                  </template>
                </span>
              </template>
            </span>
            <span class="shortcut-desc">{{ t(item.descKey) }}</span>
          </li>
        </ul>
      </section>

      <section class="about-section">
        <h3>{{ t('about.tipsTitle') }}</h3>
        <p class="about-text">{{ t('about.tipsList') }}</p>
      </section>

      <footer class="about-footer">
        <p class="about-footer-intro">
          {{ t('about.footerIntroPrefix') }}
          <a href="#" class="about-footer-link" @click.prevent="openAuthor">{{ t('about.footerIntroLinkText') }}</a>
        </p>
        <p class="about-footer-repo">
          {{ t('about.footerRepo') }}
          <a href="#" class="about-footer-link" @click.prevent="openRepo">{{ REPO_URL }}</a>
        </p>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.about-view {
  padding: 20px;
  max-width: max(600px, 80%);
  margin: 0 auto;
}

.header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 30px;
}

.about-content {
  display: flex;
  flex-direction: column;
  gap: 24px;
}

.about-section h3 {
  margin: 0 0 10px;
  font-size: 16px;
  font-weight: 600;
  color: var(--app-text-color);
}

.about-text {
  margin: 0;
  font-size: 14px;
  line-height: 1.6;
  color: var(--app-text-color);
  opacity: 0.9;
}

/* 快捷键区块 */
.shortcuts-section .shortcut-list {
  margin: 0;
  padding: 0;
  list-style: none;
}

.shortcut-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 0;
  border-bottom: 1px solid var(--app-border-color);
  flex-wrap: wrap;
}

.shortcut-row:last-child {
  border-bottom: none;
}

.shortcut-keys {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.shortcut-or {
  font-size: 12px;
  color: var(--app-text-color);
  opacity: 0.7;
}

.kbd-group {
  display: inline-flex;
  align-items: center;
  gap: 0;
}

.kbd-plus {
  margin: 0 4px;
  font-size: 11px;
  font-weight: 600;
  color: var(--app-text-color);
  opacity: 0.6;
}

.kbd {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 4px 8px;
  font-size: 12px;
  font-family: var(--app-font-family), ui-monospace, monospace;
  line-height: 1.2;
  color: var(--app-text-color);
  background: var(--app-surface-color);
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
  box-shadow: 0 1px 0 var(--app-border-color);
}

.shortcut-desc {
  font-size: 14px;
  color: var(--app-text-color);
  opacity: 0.9;
}

/* 页脚 */
.about-footer {
  margin-top: 8px;
  padding-top: 24px;
  border-top: 1px solid var(--app-border-color);
}

.about-footer-intro,
.about-footer-repo {
  margin: 0 0 8px;
  font-size: 13px;
  line-height: 1.5;
  color: var(--app-text-color);
  opacity: 0.85;
}

.about-footer-repo {
  margin-bottom: 0;
}

.about-footer-link {
  color: var(--el-color-primary);
  text-decoration: none;
}

.about-footer-link:hover {
  text-decoration: underline;
}
</style>
