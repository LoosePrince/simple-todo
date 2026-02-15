<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { useSettingsStore } from './store/settings'
import TitleBar from './components/TitleBar.vue'

const settingsStore = useSettingsStore()

function preventContextMenu(e: Event) {
  e.preventDefault()
}

onMounted(() => {
  settingsStore.applySettings()
  document.addEventListener('contextmenu', preventContextMenu)
})
onUnmounted(() => {
  document.removeEventListener('contextmenu', preventContextMenu)
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
}

/* 强制覆盖深色模式下的基础颜色 */
html.dark {
  --app-text-color: #e5e5e5;
  --app-bg-color: #1a1a1a;
  background-color: #1a1a1a;
  color: #e5e5e5;
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
  height: 100%;
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
  background-color: #2a2a2a !important;
  color: #e5e5e5 !important;
  box-shadow: 0 0 0 1px #444 inset !important;
}

.dark .el-button:not(.el-button--primary) {
  background-color: #333 !important;
  border-color: #444 !important;
  color: #e5e5e5 !important;
}

.dark .el-button:not(.el-button--primary):hover {
  background-color: #444 !important;
  border-color: #555 !important;
}

.app-content {
  flex: 1;
  overflow: auto;
}
</style>
