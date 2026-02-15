<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Pin, Minus, Maximize, X } from 'lucide-vue-next'

const appWindow = getCurrentWindow()
const pinned = ref(false)

onMounted(async () => {
  try {
    pinned.value = await appWindow.isAlwaysOnTop()
  } catch (_) {}
})

const togglePin = async () => {
  try {
    pinned.value = !pinned.value
    await appWindow.setAlwaysOnTop(pinned.value)
  } catch (_) {}
}

const minimize = () => appWindow.minimize()
const toggleMaximize = () => appWindow.toggleMaximize()
const close = () => appWindow.close()
</script>

<template>
  <div data-tauri-drag-region class="titlebar">
    <div class="title">简易代办</div>
    <div class="controls">
      <div class="control-btn pin-btn" :class="{ active: pinned }" @click="togglePin">
        <Pin :size="14" />
      </div>
      <div class="control-btn" id="btn-minimize" @click="minimize">
        <Minus :size="14" />
      </div>
      <div class="control-btn" id="btn-toggle-maximize" @click="toggleMaximize">
        <Maximize :size="14" />
      </div>
      <div class="control-btn close" id="btn-close" @click="close">
        <X :size="14" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.titlebar {
  height: 30px;
  background: #f6f6f6;
  display: flex;
  justify-content: space-between;
  align-items: center;
  user-select: none;
  border-bottom: 1px solid #ddd;
  transition: background 0.3s, border-color 0.3s;
}

.dark .titlebar {
  background: #222;
  border-bottom: 1px solid #333;
}

.title {
  padding-left: 10px;
  font-size: 12px;
  color: #666;
  pointer-events: none;
}

.dark .title {
  color: #aaa;
}

.controls {
  display: flex;
  height: 100%;
}

.control-btn {
  display: inline-flex;
  justify-content: center;
  align-items: center;
  width: 45px;
  height: 100%;
  cursor: pointer;
  transition: background 0.2s;
  color: #333;
}

.dark .control-btn {
  color: #ccc;
}

.control-btn:hover {
  background: #e5e5e5;
}

.dark .control-btn:hover {
  background: #333;
}

.control-btn.close:hover {
  background: #e81123;
  color: white;
}

.pin-btn.active {
  color: var(--el-color-primary, #409eff);
}

.dark .pin-btn.active {
  color: #58a6ff;
}
</style>
