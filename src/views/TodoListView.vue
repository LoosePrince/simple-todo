<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Edit3, ExternalLink, Info, MoreHorizontal, Plus, Settings, Trash2 } from 'lucide-vue-next'
import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import type { EditorNode } from '../editor/types'
import { collectTaskItems, countTaskProgress, normalizeDocument, stripFileSizes } from '../editor/document'
import { useSettingsStore } from '../store/settings'
import { useTodoStore } from '../store/todo'

const { t } = useI18n()
const todoStore = useTodoStore()
const settingsStore = useSettingsStore()
const router = useRouter()
const newTodoTitle = ref('')

const taskProgressMap = ref<Record<string, { done: number; total: number }>>({})

async function loadTaskProgressForTodos() {
  const dataPath = settingsStore.config.data_path
  if (!dataPath || !todoStore.todos.length) {
    taskProgressMap.value = {}
    return
  }
  const next: Record<string, { done: number; total: number }> = {}
  await Promise.all(
    todoStore.todos.map(async (item) => {
      try {
        const content = await invoke<string>('get_todo_detail', {
          dataPath,
          folderName: item.folder_name
        })
        const nodes = JSON.parse(content) as unknown
        next[item.id] = countTaskProgress(nodes)
      } catch {
        next[item.id] = { done: 0, total: 0 }
      }
    })
  )
  taskProgressMap.value = next
}

onMounted(async () => {
  try {
    await settingsStore.loadConfig()
  } catch (_) {}
  try {
    await todoStore.loadTodos(settingsStore.config.data_path)
  } catch (_) {}
  await loadTaskProgressForTodos()
})

watch(() => todoStore.todos, () => loadTaskProgressForTodos(), { deep: true })

const handleAddTodo = async () => {
  if (newTodoTitle.value.trim()) {
    await todoStore.addTodo(newTodoTitle.value)
    newTodoTitle.value = ''
  }
}

const confirmDeleteTodo = async (id: string, title: string) => {
  try {
    await ElMessageBox.confirm(
      t('todo.deleteConfirmMessage', { title }),
      t('todo.deleteConfirmTitle'),
      {
        type: 'warning',
        confirmButtonText: t('common.confirm'),
        cancelButtonText: t('common.cancel'),
        closeOnClickModal: false,
        closeOnPressEscape: false
      }
    )
    await todoStore.deleteTodo(id)
    ElMessage.success(t('todo.deleteSuccess'))
  } catch {
    // 用户取消或关闭弹窗，不做处理
  }
}

async function renameTodo(id: string, title: string) {
  try {
    const result = await ElMessageBox.prompt(t('todo.renamePrompt'), t('todo.renameTitle'), {
      inputValue: title,
      inputPlaceholder: t('todo.renamePlaceholder'),
      inputValidator: (value) => value.trim().length > 0 || t('todo.renameRequired'),
      confirmButtonText: t('common.save'),
      cancelButtonText: t('common.cancel'),
      closeOnClickModal: false
    })
    const nextTitle = String((result as { value?: unknown }).value ?? '').trim()
    if (!nextTitle || nextTitle === title) return
    await todoStore.renameTodo(id, nextTitle)
    ElMessage.success(t('todo.renameSuccess'))
  } catch {
    // 用户取消或关闭弹窗，不做处理
  }
}

async function openTodoAsQuickRecord(id: string) {
  const item = todoStore.todos.find(todo => todo.id === id)
  if (!item) {
    ElMessage.error(t('todo.notFound'))
    return
  }

  console.info('[quick-record-debug] list:open:start', { id, folderName: item.folder_name })
  invoke('quick_record_debug_log', { message: `frontend:list:open:start id=${id} folder=${item.folder_name}` }).catch(() => {})
  try {
    const query = new URLSearchParams({
      todoId: item.id,
      todoTitle: item.title,
      folderName: item.folder_name
    })
    const theme = new URLSearchParams(window.location.search).get('theme') || settingsStore.config.theme || 'light'
    const url = `index.html?theme=${encodeURIComponent(theme)}#/quick-record?${query.toString()}`
    const label = `quick-record-${Date.now()}-${item.id}`
    console.info('[quick-record-debug] list:webview:create:start', { label, url })
    invoke('quick_record_debug_log', { message: `frontend:list:webview:create:start label=${label} url=${url}` }).catch(() => {})
    const win = new WebviewWindow(label, {
      url,
      title: t('quickRecord.title'),
      width: 420,
      height: 520,
      minWidth: 160,
      minHeight: 120,
      x: 100,
      y: 100,
      decorations: false,
      alwaysOnTop: true,
      resizable: true
    })
    win.once('tauri://created', () => {
      console.info('[quick-record-debug] list:webview:create:created', { label })
      invoke('quick_record_debug_log', { message: `frontend:list:webview:create:created label=${label}` }).catch(() => {})
    })
    win.once('tauri://error', (event) => {
      console.error('[quick-record-debug] list:webview:create:error', event)
      invoke('quick_record_debug_log', { message: `frontend:list:webview:create:error label=${label} error=${JSON.stringify(event.payload)}` }).catch(() => {})
    })
  } catch (error) {
    console.error('[quick-record-debug] list:open:error', error)
    invoke('quick_record_debug_log', { message: `frontend:list:open:error id=${id} error=${String(error)}` }).catch(() => {})
    const message = error instanceof Error ? error.message : String(error)
    ElMessage.error(`${t('todo.openQuickRecordError')}: ${message}`)
  }
}

const goToDetail = (id: string) => {
  router.push(`/detail/${id}`)
}

/** 任务弹窗：当前打开的 todo 与已加载的 blocks */
const taskPopupVisible = ref(false)
const taskPopupTodoId = ref<string | null>(null)
const taskPopupFolderName = ref('')
const taskPopupTitle = ref('')
const taskPopupBlocks = ref<EditorNode[]>([])

const taskPopupTaskRows = computed(() => collectTaskItems(taskPopupBlocks.value, t('todo.emptyTaskText')))

async function openTaskPopup(item: { id: string; folder_name: string; title: string }) {
  const dataPath = settingsStore.config.data_path
  if (!dataPath) return
  try {
    const content = await invoke<string>('get_todo_detail', {
      dataPath,
      folderName: item.folder_name
    })
    const blocks = JSON.parse(content) as EditorNode[]
    taskPopupTodoId.value = item.id
    taskPopupFolderName.value = item.folder_name
    taskPopupTitle.value = item.title
    taskPopupBlocks.value = Array.isArray(blocks) ? normalizeDocument(blocks) : []
    taskPopupVisible.value = true
  } catch {
    ElMessage.error(t('todo.taskLoadError') || '加载任务失败')
  }
}

async function saveTaskPopupAndUpdateProgress() {
  const id = taskPopupTodoId.value
  const folderName = taskPopupFolderName.value
  const dataPath = settingsStore.config.data_path
  if (!id || !folderName || !dataPath) return
  try {
    const toSave = stripFileSizes(normalizeDocument(taskPopupBlocks.value))
    await invoke('save_todo_detail', {
      dataPath,
      folderName,
      content: JSON.stringify(toSave)
    })
    taskProgressMap.value = {
      ...taskProgressMap.value,
      [id]: countTaskProgress(taskPopupBlocks.value)
    }
  } catch {
    ElMessage.error(t('todo.taskSaveError') || '保存失败')
  }
}

function onTaskPopupCheckChange() {
  void saveTaskPopupAndUpdateProgress()
}

function closeTaskPopup() {
  taskPopupVisible.value = false
  taskPopupTodoId.value = null
  taskPopupFolderName.value = ''
  taskPopupTitle.value = ''
  taskPopupBlocks.value = []
}
</script>

<template>
  <div class="todo-list-view">
    <div class="header">
      <h1>{{ t('todo.listTitle') }}</h1>
      <div class="header-actions">
        <el-button circle class="header-icon-btn" :title="t('common.about')" @click="router.push('/about')">
          <Info :size="20" />
        </el-button>
        <el-button circle class="header-icon-btn" :title="t('common.settings')" @click="router.push('/settings')">
          <Settings :size="20" />
        </el-button>
      </div>
    </div>

    <div class="input-section">
      <el-input
        v-model="newTodoTitle"
        :placeholder="t('todo.placeholder')"
        @keyup.enter="handleAddTodo"
      >
        <template #append>
          <el-button @click="handleAddTodo">
            <Plus :size="16" style="margin-right: 4px" />
            {{ t('common.add') }}
          </el-button>
        </template>
      </el-input>
    </div>

    <el-scrollbar class="list-section">
      <div v-for="item in todoStore.todos" :key="item.id" class="todo-item">
        <el-checkbox
          v-model="item.status"
          true-value="completed"
          false-value="pending"
          @change="todoStore.saveTodos()"
        />
        <span
          v-if="(taskProgressMap[item.id]?.total ?? 0) > 0"
          class="task-progress clickable"
          :title="t('todo.taskProgress')"
          @click.stop="openTaskPopup(item)"
        >
          {{ taskProgressMap[item.id].done }}/{{ taskProgressMap[item.id].total }}
        </span>
        <span
          class="title"
          :class="{ completed: item.status === 'completed' }"
          @click="goToDetail(item.id)"
        >
          {{ item.title }}
        </span>
        <el-dropdown trigger="click" @command="(command: string) => {
          if (command === 'rename') renameTodo(item.id, item.title)
          if (command === 'quickRecord') openTodoAsQuickRecord(item.id)
          if (command === 'delete') confirmDeleteTodo(item.id, item.title)
        }">
          <el-button circle size="small" class="todo-more-btn" @click.stop>
            <MoreHorizontal :size="14" />
          </el-button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item command="rename">
                <Edit3 :size="14" class="todo-menu-icon" />
                {{ t('todo.rename') }}
              </el-dropdown-item>
              <el-dropdown-item command="quickRecord">
                <ExternalLink :size="14" class="todo-menu-icon" />
                {{ t('todo.openAsQuickRecord') }}
              </el-dropdown-item>
              <el-dropdown-item command="delete" divided>
                <Trash2 :size="14" class="todo-menu-icon" />
                {{ t('common.delete') }}
              </el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
      </div>
    </el-scrollbar>

    <el-dialog
      v-model="taskPopupVisible"
      :title="taskPopupTitle"
      width="360px"
      class="task-popup-dialog"
      destroy-on-close
      @close="closeTaskPopup"
    >
      <div class="task-popup-list">
        <div
          v-for="(row, index) in taskPopupTaskRows"
          :key="row.node.id ?? index"
          class="task-popup-row"
        >
          <el-checkbox
            :model-value="row.node.checked"
            @update:model-value="(val: boolean) => { row.node.checked = val; onTaskPopupCheckChange() }"
          />
          <span class="task-popup-label" :class="{ completed: row.node.checked }">{{ row.label }}</span>
        </div>
      </div>
    </el-dialog>
  </div>
</template>

<style scoped>
.todo-list-view {
  padding: 20px;
  max-width: max(600px, 80%);
  margin: 0 auto;
  height: 100%;
  display: flex;
  flex-direction: column;
}

.header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.header-actions .header-icon-btn {
  border: none;
}

.header-actions .header-icon-btn:hover,
.header-actions .header-icon-btn:focus {
  border: none;
  background: var(--el-fill-color-light);
}

.dark .header-actions .header-icon-btn:hover,
.dark .header-actions .header-icon-btn:focus {
  background: var(--el-fill-color);
}

.input-section {
  margin-bottom: 20px;
}

.list-section {
  flex: 1;
}

.todo-item {
  display: flex;
  align-items: center;
  padding: 10px;
  border-bottom: 1px solid #eee;
  gap: 10px;
}

.task-progress {
  flex-shrink: 0;
  font-size: 12px;
  color: var(--app-text-color);
  opacity: 0.8;
}

.task-progress.clickable {
  cursor: pointer;
}

.task-progress.clickable:hover {
  opacity: 1;
  text-decoration: underline;
}

.task-popup-list {
  max-height: 320px;
  overflow-y: auto;
}

.task-popup-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 0;
  border-bottom: 1px solid var(--app-border-color);
}

.task-popup-row:last-child {
  border-bottom: none;
}

.task-popup-label {
  flex: 1;
  min-width: 0;
  word-break: break-word;
}

.task-popup-label.completed {
  text-decoration: line-through;
  color: #999;
}

.todo-more-btn {
  flex-shrink: 0;
}

.todo-menu-icon {
  margin-right: 6px;
}

.title {
  flex: 1;
  cursor: pointer;
  min-width: 0;
}

.title.completed {
  text-decoration: line-through;
  color: #999;
}
</style>
