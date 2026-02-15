<script setup lang="ts">
import { ref, onMounted, watch, nextTick } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useTodoStore } from '../store/todo'
import { useSettingsStore } from '../store/settings'
import { invoke } from '@tauri-apps/api/core'
import { readFile, writeFile, mkdir, stat, readDir, remove } from '@tauri-apps/plugin-fs'
import { join } from '@tauri-apps/api/path'
import { ElMessage } from 'element-plus'
import { useI18n } from 'vue-i18n'
import { open, save as saveDialog } from '@tauri-apps/plugin-dialog'
import { openPath, revealItemInDir } from '@tauri-apps/plugin-opener'
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
/** 在工具栏按下时保存的光标所在块索引，插入图片/文件时在此位置后插入，用后置 -1 */
const insertAtIndexRef = ref(-1)
/** 图片/文件右键菜单 */
const contextMenu = ref<{ x: number; y: number; type: 'image' | 'file'; assetPath: string } | null>(null)
/** 菜单定位（避让边缘后的 left/top） */
const menuPosition = ref({ left: 0, top: 0 })
const contextMenuRef = ref<HTMLElement | null>(null)
const EDGE_PAD = 8

watch(contextMenu, (val) => {
  if (!val) return
  menuPosition.value = { left: val.x, top: val.y }
  nextTick(() => {
    const el = contextMenuRef.value
    if (!el) return
    const rect = el.getBoundingClientRect()
    let { left, top } = menuPosition.value
    if (rect.right > window.innerWidth) left = window.innerWidth - rect.width - EDGE_PAD
    if (rect.bottom > window.innerHeight) top = window.innerHeight - rect.height - EDGE_PAD
    if (rect.left < EDGE_PAD) left = EDGE_PAD
    if (rect.top < EDGE_PAD) top = EDGE_PAD
    menuPosition.value = { left, top }
  })
})

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
  await backfillFileSizes()
})

async function backfillFileSizes() {
  if (!todoItem) return
  const base = await join(settingsStore.config.data_path, todoItem.folder_name)
  const list: EditorNode[] = []
  for (const node of blocks.value) {
    if (node.type === 'file' && node.assetPath && (node.fileSize == null || node.fileSize === 0)) {
      try {
        const fullPath = await join(base, node.assetPath)
        const info = await stat(fullPath)
        list.push({ ...node, fileSize: info.size })
      } catch {
        list.push(node)
      }
    } else {
      list.push(node)
    }
  }
  blocks.value = list
}

function collectUsedAssetNames(nodes: EditorNode[]): Set<string> {
  const names = new Set<string>()
  for (const node of nodes) {
    if (node.type !== 'image' && node.type !== 'file') continue
    if (node.assetPath) {
      const name = node.assetPath.split(/[/\\]/).pop()
      if (name) names.add(name)
    }
    if (node.url) {
      try {
        const segs = decodeURIComponent(node.url).split(/[/\\]/)
        if (segs.length) names.add(segs[segs.length - 1])
      } catch (_) {}
    }
  }
  return names
}

async function sha256Hex(data: ArrayBuffer): Promise<string> {
  const buf = await crypto.subtle.digest('SHA-256', data)
  return Array.from(new Uint8Array(buf))
    .map(b => b.toString(16).padStart(2, '0'))
    .join('')
    .slice(0, 16)
}

function getExt(pathOrName: string): string {
  const m = pathOrName.match(/\.([a-zA-Z0-9]+)$/)
  return m ? m[1].toLowerCase() : 'bin'
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
    const arrayBuffer = await readFile(filePath)
    const hash = await sha256Hex(arrayBuffer)
    const ext = getExt(filePath)
    const targetFileName = `${hash}.${ext}`
    const assetPath = `assets/${targetFileName}`
    const assetsDir = await join(settingsStore.config.data_path, todoItem.folder_name, 'assets')
    const targetPath = await join(assetsDir, targetFileName)
    try {
      await stat(targetPath)
    } catch {
      await writeFile(targetPath, arrayBuffer)
    }
    const newBlock: EditorNode = {
      type: 'image',
      id: crypto.randomUUID(),
      url: convertFileSrc(targetPath),
      assetPath,
      widthPercent: 100,
      align: 'left'
    }
    const idx = insertAtIndexRef.value
    if (idx >= 0) {
      blocks.value = [...blocks.value.slice(0, idx + 1), newBlock, ...blocks.value.slice(idx + 1)]
      insertAtIndexRef.value = -1
    } else {
      blocks.value.push(newBlock)
    }
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e)
    ElMessage.error(`上传失败: ${msg}`)
  }
}

const saveDetail = async () => {
  if (!todoItem) return
  if (editorRef.value) editorRef.value.handleInput()
  const used = collectUsedAssetNames(blocks.value)
  const assetsDir = await join(settingsStore.config.data_path, todoItem.folder_name, 'assets')
  try {
    const entries = await readDir(assetsDir)
    for (const entry of entries) {
      if (entry.isFile && entry.name && !used.has(entry.name)) {
        try {
          await remove(await join(assetsDir, entry.name))
        } catch (_) {}
      }
    }
  } catch (_) {}
  await invoke('save_todo_detail', {
    dataPath: settingsStore.config.data_path,
    folderName: todoItem.folder_name,
    content: JSON.stringify(blocks.value)
  })
  ElMessage.success(t('common.saveSuccess') || '已保存')
}

const onToolbarMouseDown = () => {
  editorRef.value?.saveSelection?.()
  insertAtIndexRef.value = editorRef.value?.getCursorBlockIndex?.() ?? -1
}

const handleCommand = (cmd: string, val?: string) => {
  if (editorRef.value) editorRef.value.execCommand(cmd, val)
}

async function ensureAssetsDir() {
  if (!todoItem) return
  const assetsDir = await join(settingsStore.config.data_path, todoItem.folder_name, 'assets')
  await mkdir(assetsDir, { recursive: true })
}

const handleFileUpload = async () => {
  try {
    const selected = await open({ multiple: false })
    if (!selected || !todoItem) return
    await ensureAssetsDir()
    const filePath = selected as string
    const displayName = filePath.split(/[\\/]/).pop() || 'file'
    const arrayBuffer = await readFile(filePath)
    const hash = await sha256Hex(arrayBuffer)
    const ext = getExt(filePath)
    const targetFileName = `${hash}.${ext}`
    const assetPath = `assets/${targetFileName}`
    const assetsDir = await join(settingsStore.config.data_path, todoItem.folder_name, 'assets')
    const targetPath = await join(assetsDir, targetFileName)
    try {
      await stat(targetPath)
    } catch {
      await writeFile(targetPath, arrayBuffer)
    }
    const newBlock: EditorNode = {
      type: 'file',
      id: crypto.randomUUID(),
      url: convertFileSrc(targetPath),
      fileName: displayName,
      fileSize: arrayBuffer.byteLength,
      assetPath
    }
    const idx = insertAtIndexRef.value
    if (idx >= 0) {
      blocks.value = [...blocks.value.slice(0, idx + 1), newBlock, ...blocks.value.slice(idx + 1)]
      insertAtIndexRef.value = -1
    } else {
      blocks.value.push(newBlock)
    }
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e)
    ElMessage.error(`上传失败: ${msg}`)
  }
}

async function getAssetFullPath(assetPath: string): Promise<string> {
  if (!todoItem) return ''
  const base = await join(settingsStore.config.data_path, todoItem.folder_name)
  return join(base, assetPath)
}

function onAssetContextmenu(payload: { type: 'image' | 'file'; assetPath: string; id: string; clientX: number; clientY: number }) {
  contextMenu.value = { x: payload.clientX, y: payload.clientY, type: payload.type, assetPath: payload.assetPath }
}

function closeContextMenu() {
  contextMenu.value = null
}

async function contextMenuOpenLocation() {
  const menu = contextMenu.value
  if (!menu) return
  try {
    const fullPath = await getAssetFullPath(menu.assetPath)
    await revealItemInDir(fullPath)
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e)
    ElMessage.error(`打开文件位置失败: ${msg}`)
  }
  closeContextMenu()
}

async function contextMenuSaveAs() {
  const menu = contextMenu.value
  if (!menu) return
  const defaultName = menu.assetPath.split(/[/\\]/).pop() ?? 'file'
  try {
    const savePath = await saveDialog({ defaultPath: defaultName })
    if (!savePath) {
      closeContextMenu()
      return
    }
    const fullPath = await getAssetFullPath(menu.assetPath)
    const data = await readFile(fullPath)
    await writeFile(savePath, data)
    ElMessage.success(t('common.saveSuccess') || '已保存')
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e)
    ElMessage.error(`另存为失败: ${msg}`)
  }
  closeContextMenu()
}

async function contextMenuOpenFile() {
  const menu = contextMenu.value
  if (!menu) return
  try {
    const fullPath = await getAssetFullPath(menu.assetPath)
    await openPath(fullPath)
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e)
    ElMessage.error(`打开文件失败: ${msg}`)
  }
  closeContextMenu()
}

const handleDrop = async (e: DragEvent) => {
  e.preventDefault()
  const files = e.dataTransfer?.files
  if (!files?.length || !todoItem) return
  try {
    await ensureAssetsDir()
    const assetsDir = await join(settingsStore.config.data_path, todoItem.folder_name, 'assets')
    for (const file of Array.from(files)) {
      const arrayBuffer = await file.arrayBuffer()
      const uint8Array = new Uint8Array(arrayBuffer)
      const hash = await sha256Hex(arrayBuffer)
      const ext = getExt(file.name)
      const isImage = file.type.startsWith('image/')
      const targetFileName = `${hash}.${ext}`
      const assetPath = `assets/${targetFileName}`
      const targetPath = await join(assetsDir, targetFileName)
      try {
        await stat(targetPath)
      } catch {
        await writeFile(targetPath, uint8Array)
      }
      blocks.value.push({
        type: isImage ? 'image' : 'file',
        id: crypto.randomUUID(),
        url: convertFileSrc(targetPath),
        assetPath,
        fileName: isImage ? undefined : file.name,
        ...(isImage ? { widthPercent: 100, align: 'left' as const } : { fileSize: file.size })
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
          @contextmenu="onAssetContextmenu"
        />
      </el-scrollbar>
    </div>
    <Teleport to="body">
      <div
        v-if="contextMenu"
        class="context-menu-overlay"
        @click="closeContextMenu"
        @contextmenu.prevent
      >
        <div
          ref="contextMenuRef"
          class="context-menu"
          :style="{ left: menuPosition.left + 'px', top: menuPosition.top + 'px' }"
          @click.stop
          @contextmenu.prevent
        >
          <button type="button" class="context-menu-item" @click="contextMenuOpenLocation">打开文件位置</button>
          <button type="button" class="context-menu-item" @click="contextMenuSaveAs">另存为...</button>
          <button
            v-if="contextMenu?.type === 'file'"
            type="button"
            class="context-menu-item"
            @click="contextMenuOpenFile"
          >
            打开文件
          </button>
        </div>
      </div>
    </Teleport>
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

.context-menu-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
}

.context-menu {
  position: fixed;
  min-width: 160px;
  padding: 4px 0;
  background: var(--app-bg-color);
  border: 1px solid rgba(0, 0, 0, 0.1);
  border-radius: 8px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.12);
  z-index: 10000;
}

.dark .context-menu {
  border-color: rgba(255, 255, 255, 0.15);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
}

.context-menu-item {
  display: block;
  width: 100%;
  padding: 8px 14px;
  text-align: left;
  font-size: 13px;
  font-family: var(--app-font-family);
  color: var(--app-text-color);
  background: none;
  border: none;
  cursor: pointer;
}

.context-menu-item:hover {
  background: rgba(0, 0, 0, 0.06);
}

.dark .context-menu-item:hover {
  background: rgba(255, 255, 255, 0.1);
}
</style>
