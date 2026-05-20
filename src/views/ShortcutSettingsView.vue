<script setup lang="ts">
import { ElMessage } from 'element-plus'
import { ChevronLeft } from 'lucide-vue-next'
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useSettingsStore } from '../store/settings'

const router = useRouter()
const { t } = useI18n()
const settingsStore = useSettingsStore()
const recordingTarget = ref<keyof ShortcutFields | null>(null)

type ShortcutFields = {
  quick_record_shortcut: string
  hide_quick_record_shortcut: string
}

type ShortcutItem = {
  key: keyof ShortcutFields
  titleKey: string
  descriptionKey: string
  fallback: string
}

const shortcutItems: ShortcutItem[] = [
  {
    key: 'quick_record_shortcut',
    titleKey: 'shortcuts.openQuickRecord',
    descriptionKey: 'shortcuts.openQuickRecordDesc',
    fallback: 'Ctrl+Shift+N'
  },
  {
    key: 'hide_quick_record_shortcut',
    titleKey: 'shortcuts.toggleQuickRecord',
    descriptionKey: 'shortcuts.toggleQuickRecordDesc',
    fallback: 'Ctrl+Shift+H'
  }
]

function normalizeShortcutKey(key: string): string {
  if (key === ' ') return 'Space'
  if (key.length === 1) return key.toUpperCase()
  const aliases: Record<string, string> = {
    Escape: 'Esc',
    ArrowUp: 'Up',
    ArrowDown: 'Down',
    ArrowLeft: 'Left',
    ArrowRight: 'Right'
  }
  return aliases[key] || key
}

function getShortcutValue(item: ShortcutItem) {
  return settingsStore.config[item.key] || item.fallback
}

function startRecording(key: keyof ShortcutFields) {
  recordingTarget.value = key
}

async function saveShortcut(key: keyof ShortcutFields, shortcut: string) {
  const duplicate = shortcutItems.find(item => item.key !== key && getShortcutValue(item) === shortcut)
  if (duplicate) {
    ElMessage.warning(t('shortcuts.duplicate', { title: t(duplicate.titleKey) }))
    return
  }

  settingsStore.config[key] = shortcut
  try {
    await settingsStore.saveConfig()
    ElMessage.success(t('shortcuts.updated'))
  } catch (e) {
    ElMessage.error(`${t('shortcuts.bindFailed')}: ${e instanceof Error ? e.message : String(e)}`)
  }
}

function onShortcutKeydown(event: KeyboardEvent) {
  if (!recordingTarget.value) return
  event.preventDefault()
  event.stopPropagation()

  if (event.key === 'Escape') {
    recordingTarget.value = null
    return
  }

  const modifiers: string[] = []
  if (event.ctrlKey) modifiers.push('Ctrl')
  if (event.altKey) modifiers.push('Alt')
  if (event.shiftKey) modifiers.push('Shift')
  if (event.metaKey) modifiers.push('Meta')

  const key = normalizeShortcutKey(event.key)
  if (['Control', 'Shift', 'Alt', 'Meta'].includes(key)) return
  if (modifiers.length === 0) {
    ElMessage.warning(t('shortcuts.needModifier'))
    return
  }

  const target = recordingTarget.value
  recordingTarget.value = null
  void saveShortcut(target, [...modifiers, key].join('+'))
}
</script>

<template>
  <div class="shortcuts-view">
    <div class="header">
      <h2>{{ t('shortcuts.title') }}</h2>
      <el-button @click="router.back()">
        <ChevronLeft :size="16" style="margin-right: 4px" />
        {{ t('common.back') }}
      </el-button>
    </div>

    <div class="shortcut-list">
      <div v-for="item in shortcutItems" :key="item.key" class="shortcut-card">
        <div class="shortcut-info">
          <div class="shortcut-title">{{ t(item.titleKey) }}</div>
          <div class="shortcut-desc">{{ t(item.descriptionKey) }}</div>
        </div>
        <button
          type="button"
          class="shortcut-recorder"
          :class="{ recording: recordingTarget === item.key }"
          @click="startRecording(item.key)"
          @keydown="onShortcutKeydown"
        >
          {{ recordingTarget === item.key ? t('shortcuts.recording') : getShortcutValue(item) }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.shortcuts-view {
  padding: 20px;
  max-width: max(600px, 72%);
  margin: 0 auto;
}

.header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 24px;
}

.shortcut-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.shortcut-card {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
  padding: 16px;
  border: 1px solid var(--app-border-color);
  border-radius: 10px;
  background: var(--app-surface-color);
}

.shortcut-info {
  min-width: 0;
}

.shortcut-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--app-text-color);
}

.shortcut-desc {
  margin-top: 4px;
  font-size: 12px;
  color: rgba(0, 0, 0, 0.55);
}

.dark .shortcut-desc {
  color: rgba(255, 255, 255, 0.58);
}

.shortcut-recorder {
  min-width: 240px;
  padding: 8px 12px;
  font-family: var(--app-font-family);
  font-size: 13px;
  color: var(--app-text-color);
  background: var(--app-bg-color);
  border: 1px solid var(--app-border-color);
  border-radius: 6px;
  cursor: pointer;
  text-align: left;
}

.shortcut-recorder:hover,
.shortcut-recorder.recording {
  border-color: var(--el-color-primary, #409eff);
}

.shortcut-recorder.recording {
  color: var(--el-color-primary, #409eff);
}

@media (max-width: 560px) {
  .shortcuts-view {
    padding: 12px;
  }

  .shortcut-card {
    align-items: stretch;
    flex-direction: column;
  }

  .shortcut-recorder {
    width: 100%;
    min-width: 0;
  }
}
</style>