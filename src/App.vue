<script setup lang="ts">
import { listen } from '@tauri-apps/api/event'
import { onMounted, onUnmounted } from 'vue'
import TitleBar from './components/TitleBar.vue'
import { useSettingsStore } from './store/settings'
import { useTodoStore } from './store/todo'

const settingsStore = useSettingsStore()
const todoStore = useTodoStore()
const unlistenFns: Array<() => void> = []

function preventContextMenu(e: Event) {
  e.preventDefault()
}

onMounted(async () => {
  settingsStore.applySettings().catch(() => {})
  document.addEventListener('contextmenu', preventContextMenu)

  const unlistenConfig = await listen('config-changed', () => {
    settingsStore.loadConfig().catch(() => {})
  })
  unlistenFns.push(unlistenConfig)

  const unlistenTodos = await listen('todos-changed', () => {
    const path = settingsStore.config.data_path
    if (path) todoStore.loadTodos(path).catch(() => {})
  })
  unlistenFns.push(unlistenTodos)
})
onUnmounted(() => {
  document.removeEventListener('contextmenu', preventContextMenu)
  unlistenFns.forEach((fn) => fn())
})
</script>

<template>
  <div class="app-wrapper" :style="settingsStore.globalStyles">
    <TitleBar />
    <div class="app-content">
      <router-view></router-view>
    </div>
  </div>
</template>

<style>
:root {
  --app-font-family: Inter, system-ui, Avenir, Helvetica, Arial, sans-serif;
  --app-font-size: 14px;
  --app-text-color: #333333;
  --app-bg-color: #ffffff;
  --app-surface-color: #ffffff;
  --app-border-color: rgba(0, 0, 0, 0.1);
  --file-card-bg: rgba(0, 0, 0, 0.05);
  --file-card-border: rgba(0, 0, 0, 0.1);
  --file-size-color: rgba(0, 0, 0, 0.5);
}

/* 强制覆盖深色模式下的基础颜色 */
html.dark {
  --app-text-color: #e5e5e5;
  --app-bg-color: #1a1a1a;
  --app-surface-color: #2a2a2a;
  --app-border-color: #444;
  --file-card-bg: rgba(255, 255, 255, 0.08);
  --file-card-border: rgba(255, 255, 255, 0.15);
  --file-size-color: rgba(255, 255, 255, 0.6);
  background-color: var(--app-bg-color);
  color: var(--app-text-color);
}

body {
  margin: 0;
  overflow: hidden;
  font-family: var(--app-font-family);
  font-size: var(--app-font-size);
  color: var(--app-text-color);
  background-color: var(--app-bg-color);
}

/* 美化全局滚动条 */
::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

::-webkit-scrollbar-track {
  background: transparent;
}

::-webkit-scrollbar-thumb {
  background: rgba(0, 0, 0, 0.1);
  border-radius: 4px;
}

.dark ::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.1);
}

::-webkit-scrollbar-thumb:hover {
  background: rgba(0, 0, 0, 0.2);
}

.dark ::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.2);
}

#app {
  width: 100vw;
  height: 100vh;
  background-color: var(--app-bg-color);
}

.app-wrapper {
  width: 100%;
  display: flex;
  flex-direction: column;
  font-family: var(--app-font-family);
  font-size: var(--app-font-size);
  color: var(--app-text-color);
  background-color: var(--app-bg-color);
}

/* 强制覆盖 Element Plus 在深色模式下的某些背景 */
.dark .el-card,
.dark .el-input__wrapper,
.dark .el-textarea__inner,
.dark .el-select__wrapper {
  background-color: var(--app-surface-color) !important;
  color: var(--app-text-color) !important;
  box-shadow: 0 0 0 1px var(--app-border-color) inset !important;
}

.app-content {
  flex: 1;
  overflow: auto;
}
</style>
