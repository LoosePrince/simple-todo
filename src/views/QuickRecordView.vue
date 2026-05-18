<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Check, Minus, Pin, X } from 'lucide-vue-next'
import { onMounted, onUnmounted, ref } from 'vue'
import AdvancedEditor from '../components/AdvancedEditor.vue'
import { normalizeDocument, stripFileSizes } from '../editor/document'
import type { EditorNode } from '../editor/types'
import { useSettingsStore } from '../store/settings'
import { useTodoStore } from '../store/todo'

const appWindow = getCurrentWindow()
const settingsStore = useSettingsStore()
const todoStore = useTodoStore()

const blocks = ref<EditorNode[]>([{ type: 'p', id: crypto.randomUUID(), children: [] }])
const editorRef = ref<InstanceType<typeof AdvancedEditor> | null>(null)
const saving = ref(false)
const pinned = ref(true)

async function ensureDataReady() {
  if (!settingsStore.config.data_path) {
    await settingsStore.loadConfig()
  }
  if (!todoStore.dataPath) {
    await todoStore.loadTodos(settingsStore.config.data_path)
  }
}

async function saveAsFormalCode() {
  if (saving.value) return
  const title = await promptCodeName()
  if (!title) return

  saving.value = true
  try {
    await ensureDataReady()
    const folderName = await invoke<string>('create_todo_folder', { dataPath: settingsStore.config.data_path })
    const item = {
      id: crypto.randomUUID(),
      title,
      status: 'pending' as const,
      folder_name: folderName
    }
    const content = stripFileSizes(normalizeDocument(blocks.value))
    todoStore.todos.push(item)
    await todoStore.saveTodos()
    await invoke('save_todo_detail', {
      dataPath: settingsStore.config.data_path,
      folderName,
      content: JSON.stringify(content)
    })
    ElMessage.success('已保存为正式代码')
    await appWindow.close()
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    ElMessage.error(`保存失败: ${message}`)
  } finally {
    saving.value = false
  }
}

async function promptCodeName(): Promise<string> {
  try {
    const result = await ElMessageBox.prompt('请输入代码名', '保存快捷记录', {
      inputPlaceholder: '代码名',
      inputValue: '',
      inputValidator: (value) => value.trim().length > 0 || '代码名不能为空',
      confirmButtonText: '保存',
      cancelButtonText: '取消',
      closeOnClickModal: false
    })
    return String((result as { value?: unknown }).value ?? '').trim()
  } catch {
    return ''
  }
}

function isSaveShortcut(event: KeyboardEvent) {
  return event.ctrlKey && event.key.toLowerCase() === 's'
}

function onKeydown(event: KeyboardEvent) {
  if (isSaveShortcut(event)) {
    event.preventDefault()
    void saveAsFormalCode()
    return
  }

  const inEditor = document.activeElement?.closest('.advanced-editor-container')
  if (!inEditor) return
  if (event.ctrlKey && event.key.toLowerCase() === 'z' && !event.shiftKey) {
    event.preventDefault()
    editorRef.value?.execCommand('undo')
  } else if ((event.ctrlKey && event.key.toLowerCase() === 'y') || (event.ctrlKey && event.shiftKey && event.key.toLowerCase() === 'z')) {
    event.preventDefault()
    editorRef.value?.execCommand('redo')
  } else if (event.ctrlKey && event.key.toLowerCase() === 'b') {
    event.preventDefault()
    editorRef.value?.saveSelection?.()
    editorRef.value?.execCommand('bold')
  } else if (event.ctrlKey && event.key.toLowerCase() === 'i') {
    event.preventDefault()
    editorRef.value?.saveSelection?.()
    editorRef.value?.execCommand('italic')
  }
}

async function togglePin() {
  pinned.value = !pinned.value
  await appWindow.setAlwaysOnTop(pinned.value)
}

onMounted(async () => {
  await settingsStore.applySettings()
  await appWindow.setAlwaysOnTop(true)
  window.addEventListener('keydown', onKeydown)
})

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown)
})
</script>

<template>
  <div class="quick-record-view">
    <div data-tauri-drag-region class="quick-titlebar">
      <div data-tauri-drag-region class="quick-title-drag-region"></div>
      <div class="quick-controls">
        <button type="button" class="quick-control" :class="{ active: pinned }" title="窗口置顶" @click="togglePin">
          <Pin :size="14" />
        </button>
        <button type="button" class="quick-control" title="保存" :disabled="saving" @click="saveAsFormalCode">
          <Check :size="14" />
        </button>
        <button type="button" class="quick-control" title="最小化" @click="appWindow.minimize()">
          <Minus :size="14" />
        </button>
        <button type="button" class="quick-control close" title="关闭，不保存" @click="appWindow.close()">
          <X :size="14" />
        </button>
      </div>
    </div>

    <div class="quick-editor-shell">
      <AdvancedEditor ref="editorRef" v-model="blocks" />
    </div>
  </div>
</template>

<style scoped>
.quick-record-view {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--app-bg-color);
  color: var(--app-text-color);
  overflow: hidden;
}

.quick-titlebar {
  height: 34px;
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--app-border-color);
  background: var(--app-surface-color);
  user-select: none;
}

.quick-title-drag-region {
  flex: 1;
  height: 100%;
}

.quick-controls {
  display: flex;
  align-items: stretch;
  height: 100%;
}

.quick-control {
  width: 34px;
  height: 100%;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 0;
  color: var(--app-text-color);
  background: transparent;
  cursor: pointer;
}

.quick-control:hover {
  background: rgba(0, 0, 0, 0.08);
}

.dark .quick-control:hover {
  background: rgba(255, 255, 255, 0.1);
}

.quick-control.active {
  color: var(--el-color-primary, #409eff);
}

.quick-control.close:hover {
  color: #fff;
  background: #e81123;
}

.quick-control:disabled {
  cursor: wait;
  opacity: 0.5;
}

.quick-editor-shell {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 12px;
}

.quick-editor-shell :deep(.advanced-editor-container) {
  min-height: 100%;
}

.quick-editor-shell :deep(.advanced-editor) {
  min-height: calc(100vh - 58px);
}
</style>