<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useTodoStore } from '../store/todo'
import { useSettingsStore } from '../store/settings'
import { invoke } from '@tauri-apps/api/core'
import { readFile, writeFile, mkdir } from '@tauri-apps/plugin-fs'
import { join } from '@tauri-apps/api/path'
import { ElMessage } from 'element-plus'
import { useI18n } from 'vue-i18n'
import { open } from '@tauri-apps/plugin-dialog'
import { convertFileSrc } from '@tauri-apps/api/core'
import AdvancedEditor, { type EditorNode } from '../components/AdvancedEditor.vue'
import EditorToolbar from '../components/EditorToolbar.vue'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const todoStore = useTodoStore()
const settingsStore = useSettingsStore()

const todoId = route.params.id as string
const todoItem = todoStore.todos.find(t => t.id === todoId)

const blocks = ref<EditorNode[]>([])
const editorRef = ref<InstanceType<typeof AdvancedEditor> | null>(null)

onMounted(async () => {
  if (!todoItem) {
    router.push('/')
    return
  }
  const content = await invoke<string>('get_todo_detail', {
    dataPath: settingsStore.config.data_path,
    folderName: todoItem.folder_name
  })
  try {
    const jsonContent = JSON.parse(content)
    blocks.value = Array.isArray(jsonContent) ? (jsonContent as EditorNode[]) : []
  } catch {
    blocks.value = []
  }
  if (blocks.value.length === 0) {
    blocks.value = [{ type: 'p', id: crypto.randomUUID(), children: [] }]
  }
})

const saveDetail = async () => {
  if (!todoItem) return
  if (editorRef.value) editorRef.value.handleInput()
  await invoke('save_todo_detail', {
    dataPath: settingsStore.config.data_path,
    folderName: todoItem.folder_name,
    content: JSON.stringify(blocks.value)
  })
  ElMessage.success(t('common.saveSuccess') || '已保存')
}

const onToolbarMouseDown = () => {
  editorRef.value?.saveSelection?.()
}

const handleCommand = (cmd: string, val?: string) => {
  if (editorRef.value) editorRef.value.execCommand(cmd, val)
}

async function ensureAssetsDir() {
  if (!todoItem) return
  const assetsDir = await join(settingsStore.config.data_path, todoItem.folder_name, 'assets')
  await mkdir(assetsDir, { recursive: true })
}

const handleImageUpload = async () => {
  try {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp'] }]
    })
    if (!selected || !todoItem) return
    await ensureAssetsDir()
    const filePath = selected as string
    const fileName = `${Date.now()}-${filePath.split(/[\\/]/).pop()}`
    const arrayBuffer = await readFile(filePath)
    const targetPath = await join(settingsStore.config.data_path, todoItem.folder_name, 'assets', fileName)
    await writeFile(targetPath, arrayBuffer)
    blocks.value.push({
      type: 'image',
      id: crypto.randomUUID(),
      url: convertFileSrc(targetPath)
    })
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e)
    ElMessage.error(`上传失败: ${msg}`)
  }
}

const handleFileUpload = async () => {
  try {
    const selected = await open({ multiple: false })
    if (!selected || !todoItem) return
    await ensureAssetsDir()
    const filePath = selected as string
    const fileName = filePath.split(/[\\/]/).pop() || 'file'
    const targetFileName = `${Date.now()}-${fileName}`
    const arrayBuffer = await readFile(filePath)
    const targetPath = await join(settingsStore.config.data_path, todoItem.folder_name, 'assets', targetFileName)
    await writeFile(targetPath, arrayBuffer)
    blocks.value.push({
      type: 'file',
      id: crypto.randomUUID(),
      url: convertFileSrc(targetPath),
      fileName
    })
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e)
    ElMessage.error(`上传失败: ${msg}`)
  }
}

const handleDrop = async (e: DragEvent) => {
  e.preventDefault()
  const files = e.dataTransfer?.files
  if (!files?.length || !todoItem) return
  try {
    await ensureAssetsDir()
    for (const file of Array.from(files)) {
      const arrayBuffer = await file.arrayBuffer()
      const uint8Array = new Uint8Array(arrayBuffer)
      const fileName = `${Date.now()}-${file.name}`
      const targetPath = await join(settingsStore.config.data_path, todoItem.folder_name, 'assets', fileName)
      await writeFile(targetPath, uint8Array)
      const isImage = file.type.startsWith('image/')
      blocks.value.push({
        type: isImage ? 'image' : 'file',
        id: crypto.randomUUID(),
        url: convertFileSrc(targetPath),
        fileName: isImage ? undefined : file.name
      })
    }
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err)
    ElMessage.error(`上传失败: ${msg}`)
  }
}
</script>

<template>
  <div class="todo-detail-view" @dragover.prevent @drop="handleDrop">
    <div class="header">
      <div class="header-left">
        <el-button icon="Back" circle @click="router.back()" />
        <h2>{{ todoItem?.title }}</h2>
      </div>
      <button type="button" class="header-save-btn" @click="saveDetail">{{ t('common.save') }}</button>
    </div>

    <div class="editor-wrapper">
      <EditorToolbar
        @mousedown="onToolbarMouseDown"
        @command="handleCommand"
        @insert-image="handleImageUpload"
        @insert-file="handleFileUpload"
      />
      <el-scrollbar class="editor-scroll">
        <AdvancedEditor
          ref="editorRef"
          v-model="blocks"
        />
      </el-scrollbar>
    </div>
  </div>
</template>

<style scoped>
.todo-detail-view {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--app-bg-color);
}

.header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 20px;
  border-bottom: 1px solid rgba(0,0,0,0.05);
}

.header-left {
  display: flex;
  align-items: center;
  gap: 15px;
}

.header-left h2 {
  margin: 0;
  font-size: 18px;
}

.header-save-btn {
  padding: 6px 14px;
  font-size: 13px;
  font-family: var(--app-font-family);
  color: var(--app-text-color);
  background: rgba(0, 0, 0, 0.06);
  border: 1px solid rgba(0, 0, 0, 0.1);
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.2s, border-color 0.2s;
}

.header-save-btn:hover {
  background: rgba(0, 0, 0, 0.1);
  border-color: rgba(0, 0, 0, 0.15);
}

.dark .header-save-btn {
  background: rgba(255, 255, 255, 0.08);
  border-color: rgba(255, 255, 255, 0.15);
  color: var(--app-text-color);
}

.dark .header-save-btn:hover {
  background: rgba(255, 255, 255, 0.12);
  border-color: rgba(255, 255, 255, 0.2);
}

.editor-wrapper {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.editor-scroll {
  flex: 1;
}
</style>
