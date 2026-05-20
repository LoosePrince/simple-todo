<script setup lang="ts">
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { join } from '@tauri-apps/api/path'
import { LogicalPosition, LogicalSize, getCurrentWindow } from '@tauri-apps/api/window'
import { mkdir, stat, writeFile } from '@tauri-apps/plugin-fs'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Check, Minus, Pin, X } from 'lucide-vue-next'
import { nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import AdvancedEditor from '../components/AdvancedEditor.vue'
import EditorContextMenu from '../components/EditorContextMenu.vue'
import EditorToolbar from '../components/EditorToolbar.vue'
import { normalizeDocument, stableDocumentJson, stripFileSizes } from '../editor/document'
import { detectCodeLanguage, isLikelyCodePaste, normalizePastedText } from '../editor/paste'
import type { EditorNode } from '../editor/types'
import { useSettingsStore } from '../store/settings'
import { useTodoStore } from '../store/todo'

const appWindow = getCurrentWindow()
const { t } = useI18n()
const settingsStore = useSettingsStore()
const todoStore = useTodoStore()

const routeParams = new URLSearchParams(window.location.hash.split('?')[1] || '')
const cacheId = routeParams.get('cacheId')
const todoId = routeParams.get('todoId')
const routeTodoTitle = routeParams.get('todoTitle') || ''
const routeFolderName = routeParams.get('folderName') || ''
const isTodoQuickRecord = ref(Boolean(todoId && routeFolderName))
const todoFolderName = ref(routeFolderName)
const loadedTodoTitle = ref(routeTodoTitle)
const lastSavedJson = ref('')
const autoSaveTimer = ref<ReturnType<typeof setTimeout> | null>(null)

const blocks = ref<EditorNode[]>([{ type: 'p', id: crypto.randomUUID(), children: [] }])
const editorRef = ref<InstanceType<typeof AdvancedEditor> | null>(null)
const saving = ref(false)
const pinned = ref(true)
const tempFolderName = ref('')
const savedAsFormal = ref(false)
const editorMenu = ref<{ visible: boolean; x: number; y: number }>({ visible: false, x: 0, y: 0 })

type SavedAsset = {
  url: string
  assetPath: string
  targetPath: string
  fileName: string
  fileSize: number
}

type QuickRecordCache = {
  id: string
  content: string
  temp_folder_name: string
  x: number
  y: number
  width: number
  height: number
  pinned: boolean
}

type QuickRecordHidePayload = {
  cache_id: string
  window_label: string
}

let unlistenCacheAndClose: (() => void) | null = null
const cachedForRestore = ref(false)

function debugLog(message: string, data?: unknown) {
  const detail = data == null ? message : `${message} ${JSON.stringify(data)}`
  console.info(`[quick-record-debug] ${detail}`)
  invoke('quick_record_debug_log', { message: `frontend:${detail}` }).catch(() => {})
}

window.addEventListener('error', (event) => {
  debugLog('window_error', {
    message: event.message,
    filename: event.filename,
    lineno: event.lineno,
    colno: event.colno
  })
})

window.addEventListener('unhandledrejection', (event) => {
  debugLog('unhandled_rejection', String(event.reason))
})

async function ensureDataReady() {
  debugLog('ensureDataReady:start', { hasDataPath: Boolean(settingsStore.config.data_path), todoDataPath: todoStore.dataPath })
  if (!settingsStore.config.data_path) {
    await settingsStore.loadConfig()
  }
  if (!todoStore.dataPath) {
    await todoStore.loadTodos(settingsStore.config.data_path)
  }
  debugLog('ensureDataReady:done', { dataPath: settingsStore.config.data_path, todoCount: todoStore.todos.length })
}

function asArrayBuffer(x: ArrayBuffer | Uint8Array): ArrayBuffer {
  if (x instanceof ArrayBuffer) return x
  return x.buffer.slice(x.byteOffset, x.byteOffset + x.byteLength) as ArrayBuffer
}

async function sha256Hex(data: ArrayBuffer): Promise<string> {
  const buf = await crypto.subtle.digest('SHA-256', data)
  return Array.from(new Uint8Array(buf))
    .map(b => b.toString(16).padStart(2, '0'))
    .join('')
    .slice(0, 16)
}

function getExt(pathOrName: string, fallback = 'bin'): string {
  const m = pathOrName.match(/\.([a-zA-Z0-9]+)$/)
  return m ? m[1].toLowerCase() : fallback
}

async function ensureTempFolder(): Promise<string> {
  await ensureDataReady()
  if (isTodoQuickRecord.value) return todoFolderName.value
  if (!tempFolderName.value) {
    tempFolderName.value = await invoke<string>('create_todo_folder', { dataPath: settingsStore.config.data_path })
  }
  return tempFolderName.value
}

async function ensureTempAssetsDir(): Promise<string> {
  const folderName = await ensureTempFolder()
  const assetsDir = await join(settingsStore.config.data_path, folderName, 'assets')
  await mkdir(assetsDir, { recursive: true })
  return assetsDir
}

async function saveAssetFromFile(file: File): Promise<SavedAsset> {
  const assetsDir = await ensureTempAssetsDir()
  const arrayBuffer = await file.arrayBuffer()
  const buffer = asArrayBuffer(arrayBuffer)
  const isImage = file.type.startsWith('image/')
  const fallbackExt = isImage ? (file.type.split('/')[1] || 'png').toLowerCase() : 'bin'
  const ext = getExt(file.name || '', fallbackExt)
  const hash = await sha256Hex(buffer)
  const targetFileName = `${hash}.${ext}`
  const assetPath = `assets/${targetFileName}`
  const targetPath = await join(assetsDir, targetFileName)

  let fileExists = false
  try {
    await stat(targetPath)
    fileExists = true
  } catch {
    fileExists = false
  }

  if (!fileExists) {
    await writeFile(targetPath, new Uint8Array(buffer))
    await stat(targetPath)
  }

  return {
    url: convertFileSrc(targetPath),
    assetPath,
    targetPath,
    fileName: file.name || 'file',
    fileSize: buffer.byteLength
  }
}

async function handleEditorPasteFiles(payload: { files: File[]; text: string }) {
  const newNodes: EditorNode[] = []

  try {
    for (const file of payload.files || []) {
      const asset = await saveAssetFromFile(file)
      if (file.type.startsWith('image/')) {
        newNodes.push({
          type: 'image',
          id: crypto.randomUUID(),
          url: asset.url,
          assetPath: asset.assetPath,
          widthPercent: 100,
          align: 'left'
        })
      } else {
        newNodes.push({
          type: 'file',
          id: crypto.randomUUID(),
          url: asset.url,
          fileName: asset.fileName,
          assetPath: asset.assetPath,
          fileSize: asset.fileSize
        })
      }
    }

    if (payload.text && payload.text.trim().length > 0) {
      const normalized = normalizePastedText(payload.text)
      if (isLikelyCodePaste(normalized)) {
        newNodes.push({
          type: 'code',
          id: crypto.randomUUID(),
          content: normalized,
          language: detectCodeLanguage(normalized)
        })
      } else {
        for (const line of normalized.split('\n')) {
          newNodes.push({
            type: 'p',
            id: crypto.randomUUID(),
            children: line ? [{ type: 'text', value: line }] : []
          })
        }
      }
    }

    if (newNodes.length) editorRef.value?.insertNodesAtSelection?.(newNodes)
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    ElMessage.error(`${t('common.pasteFailed')}: ${message}`)
  }
}

async function saveAsFormalCode() {
  if (isTodoQuickRecord.value) {
    await saveTodoQuickRecord(false)
    return
  }
  if (saving.value) return
  const title = await promptCodeName()
  if (!title) return

  saving.value = true
  try {
    await ensureDataReady()
    const folderName = tempFolderName.value || await invoke<string>('create_todo_folder', { dataPath: settingsStore.config.data_path })
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
    savedAsFormal.value = true
    ElMessage.success(t('quickRecord.savedAsFormal'))
    await appWindow.close()
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    ElMessage.error(`${t('common.saveFailed')}: ${message}`)
  } finally {
    saving.value = false
  }
}

async function promptCodeName(): Promise<string> {
  try {
    const result = await ElMessageBox.prompt(t('quickRecord.promptCodeName'), t('quickRecord.promptTitle'), {
      inputPlaceholder: t('quickRecord.codeNamePlaceholder'),
      inputValue: '',
      inputValidator: (value) => value.trim().length > 0 || t('quickRecord.codeNameRequired'),
      confirmButtonText: t('common.save'),
      cancelButtonText: t('common.cancel'),
      closeOnClickModal: false
    })
    return String((result as { value?: unknown }).value ?? '').trim()
  } catch {
    return ''
  }
}

async function loadTodoQuickRecord() {
  debugLog('loadTodoQuickRecord:start', {
    todoId,
    todoFolderName: todoFolderName.value,
    title: loadedTodoTitle.value,
    href: window.location.href,
    hash: window.location.hash,
    search: window.location.search
  })
  if (!todoId || !todoFolderName.value) {
    debugLog('loadTodoQuickRecord:missing_route_data', { todoId, folderName: todoFolderName.value })
    return
  }
  await settingsStore.loadConfig()
  debugLog('loadTodoQuickRecord:config_loaded', { dataPath: settingsStore.config.data_path, theme: settingsStore.config.theme })
  tempFolderName.value = todoFolderName.value
  try {
    debugLog('loadTodoQuickRecord:get_detail_start', { folderName: todoFolderName.value })
    const content = await invoke<string>('get_todo_detail', {
      dataPath: settingsStore.config.data_path,
      folderName: todoFolderName.value
    })
    debugLog('loadTodoQuickRecord:get_detail_done', { contentLength: content.length })
    const parsed = JSON.parse(content)
    blocks.value = Array.isArray(parsed) ? normalizeDocument(parsed as EditorNode[]) : []
    debugLog('loadTodoQuickRecord:normalized', { blockCount: blocks.value.length })
  } catch (error) {
    debugLog('loadTodoQuickRecord:error', String(error))
    blocks.value = []
  }
  if (blocks.value.length === 0) {
    blocks.value = [{ type: 'p', id: crypto.randomUUID(), children: [] }]
  }
  lastSavedJson.value = stableDocumentJson(blocks.value)
  debugLog('loadTodoQuickRecord:done', { blockCount: blocks.value.length, savedLength: lastSavedJson.value.length })
}

async function saveTodoQuickRecord(silent = true) {
  debugLog('saveTodoQuickRecord:start', { silent, isTodoQuickRecord: isTodoQuickRecord.value, folderName: todoFolderName.value, saving: saving.value })
  if (!isTodoQuickRecord.value || !todoFolderName.value || saving.value) return
  saving.value = true
  try {
    await ensureDataReady()
    const content = stripFileSizes(normalizeDocument(blocks.value))
    await invoke('save_todo_detail', {
      dataPath: settingsStore.config.data_path,
      folderName: todoFolderName.value,
      content: JSON.stringify(content)
    })
    lastSavedJson.value = stableDocumentJson(content)
    debugLog('saveTodoQuickRecord:done', { savedLength: lastSavedJson.value.length })
    if (!silent) ElMessage.success(t('common.saveSuccess'))
  } catch (error) {
    debugLog('saveTodoQuickRecord:error', String(error))
    const message = error instanceof Error ? error.message : String(error)
    ElMessage.error(`${t('common.saveFailed')}: ${message}`)
  } finally {
    saving.value = false
  }
}

function scheduleTodoQuickRecordSave() {
  if (!isTodoQuickRecord.value || !lastSavedJson.value) return
  if (autoSaveTimer.value) clearTimeout(autoSaveTimer.value)
  autoSaveTimer.value = setTimeout(() => {
    autoSaveTimer.value = null
    if (stableDocumentJson(blocks.value) !== lastSavedJson.value) {
      void saveTodoQuickRecord(true)
    }
  }, 1000)
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

function onToolbarMouseDown() {
  editorRef.value?.saveSelection?.()
}

function handleCommand(command: string, value?: string) {
  editorRef.value?.saveSelection?.()
  editorRef.value?.execCommand(command, value)
}

function handleInsertTask() {
  editorRef.value?.insertTaskListAtSelection?.()
}

function handleInsertCode() {
  editorRef.value?.insertCodeBlock?.()
}

function handleInsertMarkdown() {
  editorRef.value?.insertMarkdownBlock?.()
}

function handleInsertCanvas() {
  editorRef.value?.insertCanvasBlock?.()
}

function handleInsertFold() {
  editorRef.value?.insertFoldBlock?.()
}

function openEditorContextMenu(event: MouseEvent) {
  editorRef.value?.saveSelection?.()
  editorMenu.value = { visible: true, x: event.clientX, y: event.clientY }
}

function closeEditorContextMenu() {
  editorMenu.value.visible = false
}

function runEditorContextAction(action: string, value?: string) {
  closeEditorContextMenu()
  editorRef.value?.saveSelection?.()
  if (action === 'insertImage') return
  if (action === 'insertFile') return
  if (action === 'insertTask') return handleInsertTask()
  if (action === 'insertCode') return handleInsertCode()
  if (action === 'insertMarkdown') return handleInsertMarkdown()
  if (action === 'insertCanvas') return handleInsertCanvas()
  if (action === 'insertFold') return handleInsertFold()
  editorRef.value?.execCommand(action, value)
}

watch(blocks, () => scheduleTodoQuickRecordSave(), { deep: true })

async function saveQuickRecordCacheAndClose(cacheIdToSave: string) {
  if (cachedForRestore.value) return
  cachedForRestore.value = true
  try {
    await ensureDataReady()
    const folderName = tempFolderName.value || await ensureTempFolder()
    const scaleFactor = await appWindow.scaleFactor()
    const size = await appWindow.innerSize()
    const position = await appWindow.outerPosition()
    await invoke<string>('save_quick_record_cache', {
      cache: {
        id: cacheIdToSave,
        content: JSON.stringify(stripFileSizes(normalizeDocument(blocks.value))),
        temp_folder_name: folderName,
        x: position.x / scaleFactor,
        y: position.y / scaleFactor,
        width: size.width / scaleFactor,
        height: size.height / scaleFactor,
        pinned: pinned.value
      }
    })
  } catch (error) {
    cachedForRestore.value = false
    const message = error instanceof Error ? error.message : String(error)
    ElMessage.error(`${t('quickRecord.cacheFailed')}: ${message}`)
    return
  }
  await appWindow.close()
}

async function loadQuickRecordCache() {
  if (!cacheId) return
  const cache = await invoke<QuickRecordCache | null>('get_quick_record_cache', { id: cacheId })
  if (!cache) return

  tempFolderName.value = cache.temp_folder_name
  pinned.value = cache.pinned
  await appWindow.setSize(new LogicalSize(cache.width, cache.height))
  await appWindow.setPosition(new LogicalPosition(cache.x, cache.y))
  await appWindow.setAlwaysOnTop(cache.pinned)

  try {
    const content = JSON.parse(cache.content)
    blocks.value = Array.isArray(content) ? normalizeDocument(content as EditorNode[]) : []
  } catch {
    blocks.value = []
  }
  if (blocks.value.length === 0) {
    blocks.value = [{ type: 'p', id: crypto.randomUUID(), children: [] }]
  }
  await invoke('delete_quick_record_cache', { id: cache.id })
  await nextTick()
}

async function cleanupTempFolder() {
  if (!tempFolderName.value || savedAsFormal.value || cachedForRestore.value || isTodoQuickRecord.value) return
  try {
    await invoke('delete_todo_folder', {
      dataPath: settingsStore.config.data_path,
      folderName: tempFolderName.value
    })
  } catch (_) {}
}

onMounted(async () => {
  debugLog('mounted:start', {
    isTodoQuickRecord: isTodoQuickRecord.value,
    todoId,
    routeFolderName,
    routeTodoTitle,
    href: window.location.href,
    hash: window.location.hash,
    search: window.location.search
  })
  if (isTodoQuickRecord.value) {
    await loadTodoQuickRecord()
  } else {
    debugLog('mounted:normal_quick_record_start')
    await settingsStore.applySettings()
    await loadQuickRecordCache()
  }
  debugLog('mounted:set_always_on_top_start')
  await appWindow.setAlwaysOnTop(true)
  debugLog('mounted:set_always_on_top_done')
  unlistenCacheAndClose = await listen<QuickRecordHidePayload>('quick-record-cache-and-close', (event) => {
    if (event.payload.window_label !== appWindow.label) return
    if (isTodoQuickRecord.value) {
      void appWindow.close()
      return
    }
    void saveQuickRecordCacheAndClose(event.payload.cache_id)
  })
  window.addEventListener('keydown', onKeydown)
  debugLog('mounted:done')
})

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown)
  if (autoSaveTimer.value) clearTimeout(autoSaveTimer.value)
  if (isTodoQuickRecord.value && stableDocumentJson(blocks.value) !== lastSavedJson.value) {
    void saveTodoQuickRecord(true)
  }
  unlistenCacheAndClose?.()
  void cleanupTempFolder()
})
</script>

<template>
  <div class="quick-record-view">
    <div data-tauri-drag-region class="quick-titlebar">
      <div data-tauri-drag-region class="quick-title-drag-region">
        <span v-if="isTodoQuickRecord" data-tauri-drag-region class="quick-title-text">{{ loadedTodoTitle }}</span>
      </div>
      <div class="quick-controls">
        <button type="button" class="quick-control" :class="{ active: pinned }" :title="t('quickRecord.pin')" @click="togglePin">
          <Pin :size="14" />
        </button>
        <button type="button" class="quick-control" :title="isTodoQuickRecord ? t('common.save') : t('quickRecord.save')" :disabled="saving" @click="saveAsFormalCode">
          <Check :size="14" />
        </button>
        <button type="button" class="quick-control" :title="t('quickRecord.minimize')" @click="appWindow.minimize()">
          <Minus :size="14" />
        </button>
        <button type="button" class="quick-control close" :title="t('quickRecord.closeWithoutSaving')" @click="appWindow.close()">
          <X :size="14" />
        </button>
      </div>
    </div>

    <div class="quick-hidden-toolbar" aria-hidden="true">
      <EditorToolbar
        @mousedown="onToolbarMouseDown"
        @command="handleCommand"
        @insert-task="handleInsertTask"
        @insert-code="handleInsertCode"
        @insert-markdown="handleInsertMarkdown"
        @insert-canvas="handleInsertCanvas"
        @insert-fold="handleInsertFold"
      />
    </div>

    <div class="quick-editor-shell" @contextmenu.prevent="openEditorContextMenu">
      <AdvancedEditor ref="editorRef" v-model="blocks" @paste-files="handleEditorPasteFiles" />
    </div>

    <EditorContextMenu
      :visible="editorMenu.visible"
      :x="editorMenu.x"
      :y="editorMenu.y"
      :allow-assets="false"
      @close="closeEditorContextMenu"
      @action="runEditorContextAction"
    />
  </div>
</template>

<style scoped>
.quick-record-view {
  width: 100%;
  min-width: 0;
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--app-bg-color);
  color: var(--app-text-color);
  overflow: hidden;
}

.quick-titlebar {
  min-width: 0;
  height: 28px;
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--app-border-color);
  background: var(--app-surface-color);
  user-select: none;
}

.quick-title-drag-region {
  flex: 1 1 0;
  min-width: 0;
  height: 100%;
  display: flex;
  align-items: center;
  padding: 0 8px;
}

.quick-title-text {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  opacity: 0.8;
}

.quick-controls {
  display: flex;
  flex: 0 1 auto;
  align-items: stretch;
  min-width: 0;
  height: 100%;
}

.quick-control {
  width: 28px;
  min-width: 0;
  flex: 1 1 28px;
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

.quick-hidden-toolbar {
  position: fixed;
  width: 0;
  height: 0;
  overflow: hidden;
  pointer-events: none;
  opacity: 0;
}

.quick-hidden-toolbar :deep(.editor-toolbar) {
  display: none;
}

.quick-editor-shell {
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: auto;
  padding: 12px;
}

@media (max-width: 259px), (max-height: 179px) {
  .quick-titlebar {
    height: 24px;
  }

  .quick-control {
    width: 24px;
  }

  .quick-control :deep(svg) {
    width: 12px;
    height: 12px;
  }

  .quick-editor-shell {
    padding: 0;
  }

  .quick-editor-shell :deep(.advanced-editor) {
    padding: 0 1.5ch;
    min-height: calc(100vh - 24px);
  }
}

.quick-editor-shell :deep(.advanced-editor-container) {
  min-width: 0;
  min-height: 100%;
}

.quick-editor-shell :deep(.advanced-editor) {
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  min-height: calc(100vh - 58px);
  overflow-wrap: anywhere;
}
</style>