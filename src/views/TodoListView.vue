<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { ElMessage } from 'element-plus'
import { Info, Plus, Settings, Trash2 } from 'lucide-vue-next'
import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import type { EditorNode } from '../components/AdvancedEditor.vue'
import { useSettingsStore } from '../store/settings'
import { useTodoStore } from '../store/todo'

const { t } = useI18n()
const todoStore = useTodoStore()
const settingsStore = useSettingsStore()
const router = useRouter()
const newTodoTitle = ref('')

/** 从节点树抽取纯文本（用于任务项标签） */
function getTextFromEditorNode(node: EditorNode): string {
  if (node.type === 'text') return node.value ?? ''
  if ('children' in node && Array.isArray(node.children)) {
    return node.children.map(getTextFromEditorNode).join('')
  }
  return ''
}

/** 从 blocks 中收集所有 taskItem，用于弹窗列表 */
function collectTaskItems(blocks: EditorNode[]): { node: EditorNode & { type: 'taskItem'; checked: boolean; children: EditorNode[] }; label: string }[] {
  const out: { node: EditorNode & { type: 'taskItem'; checked: boolean; children: EditorNode[] }; label: string }[] = []
  function walk(nodes: EditorNode[]) {
    for (const n of nodes) {
      if (n.type === 'taskItem') {
        const label = getTextFromEditorNode(n).trim() || '(无文字)'
        out.push({ node: n, label })
      }
      if ('children' in n && Array.isArray(n.children)) walk(n.children)
    }
  }
  walk(blocks)
  return out
}

/** 从详情 JSON 的节点树中统计 taskItem 的完成数/总数 */
function countTaskProgress(nodes: unknown): { done: number; total: number } {
  let done = 0
  let total = 0
  function walk(obj: unknown) {
    if (!obj || typeof obj !== 'object') return
    const node = obj as { type?: string; checked?: boolean; children?: unknown[] }
    if (Array.isArray(node)) {
      node.forEach(walk)
      return
    }
    if (node.type === 'taskItem') {
      total += 1
      if (node.checked === true) done += 1
    }
    if (Array.isArray(node.children)) node.children.forEach(walk)
  }
  if (Array.isArray(nodes)) nodes.forEach(walk)
  else walk(nodes)
  return { done, total }
}

/** 保存前去掉 file 节点的 fileSize（与详情页一致） */
function stripFileSizes(nodes: EditorNode[]): EditorNode[] {
  return nodes.map((node) => {
    if (node.type === 'file') {
      const { fileSize: _, ...rest } = node
      return rest as EditorNode
    }
    if ('children' in node && Array.isArray(node.children)) {
      return { ...node, children: stripFileSizes(node.children) }
    }
    return node
  })
}

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

const goToDetail = (id: string) => {
  router.push(`/detail/${id}`)
}

/** 任务弹窗：当前打开的 todo 与已加载的 blocks */
const taskPopupVisible = ref(false)
const taskPopupTodoId = ref<string | null>(null)
const taskPopupFolderName = ref('')
const taskPopupTitle = ref('')
const taskPopupBlocks = ref<EditorNode[]>([])

const taskPopupTaskRows = computed(() => collectTaskItems(taskPopupBlocks.value))

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
    taskPopupBlocks.value = Array.isArray(blocks) ? blocks : []
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
    const toSave = stripFileSizes(taskPopupBlocks.value)
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
        <el-button
          type="danger"
          circle
          size="small"
          @click="todoStore.deleteTodo(item.id)"
        >
          <Trash2 :size="14" />
        </el-button>
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
