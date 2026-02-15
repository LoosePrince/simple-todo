<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, computed } from 'vue'
import { AlignLeft, AlignCenter, AlignRight } from 'lucide-vue-next'

/** 树形节点：类似 HTML 序列化为 JSON，支持子节点，加粗/斜体等内联格式保存在子节点中 */
export type EditorNode =
  | { type: 'text'; value: string }
  | { type: 'p'; id?: string; children: EditorNode[] }
  | { type: 'h1'; id?: string; children: EditorNode[] }
  | { type: 'h2'; id?: string; children: EditorNode[] }
  | { type: 'strong'; children: EditorNode[] }
  | { type: 'em'; children: EditorNode[] }
  | { type: 'ul'; id?: string; children: EditorNode[] }
  | { type: 'ol'; id?: string; children: EditorNode[] }
  | { type: 'li'; id?: string; children: EditorNode[] }
  | { type: 'image'; id: string; url: string; assetPath?: string; widthPercent?: number; align?: 'left' | 'center' | 'right' }
  | { type: 'file'; id: string; url: string; fileName?: string; fileSize?: number; assetPath?: string }

const props = defineProps<{
  modelValue: EditorNode[]
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: EditorNode[]): void
  (e: 'upload-image'): void
  (e: 'upload-file'): void
  (e: 'contextmenu', payload: { type: 'image' | 'file'; assetPath: string; id: string; clientX: number; clientY: number }): void
}>()

const editorRef = ref<HTMLDivElement | null>(null)
const isInternalUpdate = ref(false)
/** 选区在编辑器内时更新的“光标所在块”索引，用于失焦后仍能在该位置插入 */
const lastCursorBlockIndex = ref(-1)
const selectedImageId = ref<string | null>(null)
const selectedImageRect = ref({ top: 0, left: 0 })
const imageToolbarRef = ref<HTMLElement | null>(null)
const IMAGE_TOOLBAR_APPROX_HEIGHT = 44
const IMAGE_TOOLBAR_APPROX_WIDTH = 220
const IMAGE_TOOLBAR_PADDING = 8

const genId = () => crypto.randomUUID()

const escapeHtml = (text: string) => {
  const div = document.createElement('div')
  div.textContent = text
  return div.innerHTML
}

const formatFileSize = (bytes: number): string => {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

// 树形节点转 HTML
const nodeToHtml = (node: EditorNode): string => {
  if (node.type === 'text') return escapeHtml(node.value)
  if (node.type === 'image') {
    const w = node.widthPercent ?? 100
    const align = node.align ?? 'left'
    const ap = node.assetPath ?? ''
    return `<span class="image-block-wrapper editor-block" data-id="${node.id}" data-align="${align}" data-width-percent="${w}" data-asset-path="${escapeHtml(ap)}" style="display: block; text-align: ${align}; margin: 8px 0;"><img class="image-block" data-id="${node.id}" data-width-percent="${w}" data-align="${align}" data-asset-path="${escapeHtml(ap)}" src="${node.url}" style="width: ${w}%; max-width: 100%; display: inline-block; vertical-align: middle; border-radius: 8px; cursor: pointer;" onerror="console.error('Image load failed:', this.src)" /></span>`
  }
  if (node.type === 'file') {
    const name = escapeHtml(node.fileName ?? '')
    const url = node.url ?? ''
    const sizeStr = node.fileSize != null ? formatFileSize(node.fileSize) : ''
    const ap = node.assetPath ?? ''
    return `<span class="editor-block file-block" data-id="${node.id}" data-url="${escapeHtml(url)}" data-file-size="${node.fileSize ?? ''}" data-asset-path="${escapeHtml(ap)}" contenteditable="false" style="display: inline-block; vertical-align: middle; margin: 0 5px;"><span class="file-card"><span class="file-icon">📄</span><span class="file-name">${name}</span>${sizeStr ? `<span class="file-size">${escapeHtml(sizeStr)}</span>` : ''}</span></span>`
  }
  if (node.type === 'strong') return `<strong>${(node.children || []).map(nodeToHtml).join('')}</strong>`
  if (node.type === 'em') return `<em>${(node.children || []).map(nodeToHtml).join('')}</em>`
  if (node.type === 'li') {
    const inner = (node.children || []).map(nodeToHtml).join('') || '<br>'
    return `<li class="editor-block" data-id="${node.id ?? genId()}">${inner}</li>`
  }
  if (node.type === 'ul' || node.type === 'ol') {
    const inner = (node.children || []).map(nodeToHtml).join('') || '<li><br></li>'
    return `<${node.type} class="editor-block list-block" data-id="${node.id ?? genId()}">${inner}</${node.type}>`
  }
  if (node.type === 'p' || node.type === 'h1' || node.type === 'h2') {
    const inner = (node.children || []).map(nodeToHtml).join('') || '<br>'
    return `<${node.type} class="editor-block" data-id="${node.id ?? genId()}">${inner}</${node.type}>`
  }
  return ''
}

const nodesToHtml = (nodes: EditorNode[]) => nodes.map(nodeToHtml).join('')

// DOM 转树形节点（保留加粗、斜体、列表等子节点）
const collectChildren = (el: HTMLElement): EditorNode[] => {
  const out: EditorNode[] = []
  for (const node of el.childNodes) {
    if (node.nodeType === Node.TEXT_NODE) {
      const v = (node.textContent ?? '').replace(/\u00A0/g, ' ')
      if (v.length) out.push({ type: 'text', value: v })
      continue
    }
    if (node.nodeType !== Node.ELEMENT_NODE) continue
    const child = node as HTMLElement
    const tag = child.tagName.toLowerCase()
    const id = child.getAttribute('data-id') ?? genId()

    if (child.classList.contains('image-block-wrapper')) {
      const img = child.querySelector('img.image-block')
      if (img) {
        const mid = img.getAttribute('data-id') ?? id
        const url = (img as HTMLImageElement).src
        const wp = img.getAttribute('data-width-percent')
        const al = img.getAttribute('data-align')
        const ap = img.getAttribute('data-asset-path') ?? child.getAttribute('data-asset-path') ?? ''
        out.push({
          type: 'image',
          id: mid,
          url,
          assetPath: ap || undefined,
          widthPercent: wp != null && wp !== '' ? parseInt(wp, 10) : undefined,
          align: (al === 'center' || al === 'right' ? al : 'left') as 'left' | 'center' | 'right'
        })
      }
    } else if (tag === 'img') {
      const wp = child.getAttribute('data-width-percent')
      const al = child.getAttribute('data-align')
      const ap = child.getAttribute('data-asset-path') ?? ''
      out.push({
        type: 'image',
        id,
        url: (child as HTMLImageElement).src,
        assetPath: ap || undefined,
        widthPercent: wp != null && wp !== '' ? parseInt(wp, 10) : undefined,
        align: (al === 'center' || al === 'right' ? al : 'left') as 'left' | 'center' | 'right'
      })
    } else if (child.classList.contains('file-block')) {
      const name = child.querySelector('.file-name')?.textContent ?? ''
      const url = child.getAttribute('data-url') ?? ''
      const fs = child.getAttribute('data-file-size')
      const ap = child.getAttribute('data-asset-path') ?? ''
      out.push({ type: 'file', id, url, fileName: name, fileSize: fs != null && fs !== '' ? parseInt(fs, 10) : undefined, assetPath: ap || undefined })
    } else if (tag === 'strong' || tag === 'b') {
      out.push({ type: 'strong', children: collectChildren(child) })
    } else if (tag === 'em' || tag === 'i') {
      out.push({ type: 'em', children: collectChildren(child) })
    } else if (tag === 'p' || tag === 'h1' || tag === 'h2') {
      out.push({ type: tag as 'p' | 'h1' | 'h2', id, children: collectChildren(child) })
    } else if (tag === 'ul' || tag === 'ol') {
      const listChildren: EditorNode[] = []
      child.querySelectorAll(':scope > li').forEach(li => {
        listChildren.push({ type: 'li', id: (li as HTMLElement).getAttribute('data-id') ?? genId(), children: collectChildren(li as HTMLElement) })
      })
      if (listChildren.length === 0) listChildren.push({ type: 'li', id: genId(), children: [] })
      out.push({ type: tag as 'ul' | 'ol', id, children: listChildren })
    } else if (tag === 'li') {
      out.push({ type: 'li', id, children: collectChildren(child) })
    } else if (tag === 'ul' || tag === 'ol') {
      const listChildren: EditorNode[] = []
      child.querySelectorAll(':scope > li').forEach(li => {
        listChildren.push({ type: 'li', id: (li as HTMLElement).getAttribute('data-id') ?? genId(), children: collectChildren(li as HTMLElement) })
      })
      if (listChildren.length === 0) listChildren.push({ type: 'li', id: genId(), children: [] })
      out.push({ type: tag as 'ul' | 'ol', id, children: listChildren })
    } else if (tag === 'div' || tag === 'span') {
      out.push(...collectChildren(child))
    } else if (tag !== 'br') {
      const text = child.textContent
      if (text) out.push({ type: 'text', value: text })
    }
  }
  return out
}

const domToNodes = (): EditorNode[] => {
  if (!editorRef.value) return []
  const roots: EditorNode[] = []
  const el = editorRef.value
  for (const node of el.childNodes) {
    if (node.nodeType === Node.TEXT_NODE) {
      const v = (node.textContent ?? '').replace(/\u00A0/g, ' ').trim()
      if (v) roots.push({ type: 'p', id: genId(), children: [{ type: 'text', value: v }] })
      continue
    }
    if (node.nodeType !== Node.ELEMENT_NODE) continue
    const child = node as HTMLElement
    const tag = child.tagName.toLowerCase()
    const id = child.getAttribute('data-id') ?? genId()

    if (child.classList.contains('image-block-wrapper')) {
      const img = child.querySelector('img.image-block')
      if (img) {
        const mid = img.getAttribute('data-id') ?? id
        const url = (img as HTMLImageElement).src
        const wp = img.getAttribute('data-width-percent')
        const al = img.getAttribute('data-align')
        const ap = img.getAttribute('data-asset-path') ?? child.getAttribute('data-asset-path') ?? ''
        roots.push({
          type: 'image',
          id: mid,
          url,
          assetPath: ap || undefined,
          widthPercent: wp != null && wp !== '' ? parseInt(wp, 10) : undefined,
          align: (al === 'center' || al === 'right' ? al : 'left') as 'left' | 'center' | 'right'
        })
      }
    } else if (tag === 'img') {
      const wp = child.getAttribute('data-width-percent')
      const al = child.getAttribute('data-align')
      const ap = child.getAttribute('data-asset-path') ?? ''
      roots.push({
        type: 'image',
        id,
        url: (child as HTMLImageElement).src,
        assetPath: ap || undefined,
        widthPercent: wp != null && wp !== '' ? parseInt(wp, 10) : undefined,
        align: (al === 'center' || al === 'right' ? al : 'left') as 'left' | 'center' | 'right'
      })
    } else if (child.classList.contains('file-block')) {
      const name = child.querySelector('.file-name')?.textContent ?? ''
      const url = child.getAttribute('data-url') ?? ''
      const fs = child.getAttribute('data-file-size')
      const ap = child.getAttribute('data-asset-path') ?? ''
      roots.push({ type: 'file', id, url, fileName: name, fileSize: fs != null && fs !== '' ? parseInt(fs, 10) : undefined, assetPath: ap || undefined })
    } else if (tag === 'p' || tag === 'h1' || tag === 'h2') {
      roots.push({ type: tag as 'p' | 'h1' | 'h2', id, children: collectChildren(child) })
    } else if (tag === 'ul' || tag === 'ol') {
      const listChildren: EditorNode[] = []
      child.querySelectorAll(':scope > li').forEach(li => {
        listChildren.push({ type: 'li', id: (li as HTMLElement).getAttribute('data-id') ?? genId(), children: collectChildren(li as HTMLElement) })
      })
      if (listChildren.length === 0) listChildren.push({ type: 'li', id: genId(), children: [] })
      roots.push({ type: tag as 'ul' | 'ol', id, children: listChildren })
    } else if (tag === 'div') {
      const fromDiv = domToNodesFromContainer(child)
      roots.push(...(fromDiv.length > 0 ? fromDiv : [{ type: 'p', id: genId(), children: [] }]))
    } else if (tag === 'br') {
      roots.push({ type: 'p', id: genId(), children: [] })
    } else {
      const text = child.textContent?.trim()
      if (text) roots.push({ type: 'p', id: genId(), children: [{ type: 'text', value: text }] })
    }
  }
  return roots.length > 0 ? roots : [{ type: 'p', id: genId(), children: [] }]
}

function domToNodesFromContainer(div: HTMLElement): EditorNode[] {
  const roots: EditorNode[] = []
  for (const node of div.childNodes) {
    if (node.nodeType === Node.TEXT_NODE) {
      const v = (node.textContent ?? '').replace(/\u00A0/g, ' ').trim()
      if (v) roots.push({ type: 'p', id: genId(), children: [{ type: 'text', value: v }] })
    } else if (node.nodeType === Node.ELEMENT_NODE) {
      const el = node as HTMLElement
      const tag = el.tagName.toLowerCase()
      const id = el.getAttribute('data-id') ?? genId()
      if (el.classList.contains('image-block-wrapper')) {
        const img = el.querySelector('img.image-block')
        if (img) {
          const mid = img.getAttribute('data-id') ?? id
          const url = (img as HTMLImageElement).src
          const wp = img.getAttribute('data-width-percent')
          const al = img.getAttribute('data-align')
          const ap = img.getAttribute('data-asset-path') ?? el.getAttribute('data-asset-path') ?? ''
          roots.push({
            type: 'image',
            id: mid,
            url,
            assetPath: ap || undefined,
            widthPercent: wp != null && wp !== '' ? parseInt(wp, 10) : undefined,
            align: (al === 'center' || al === 'right' ? al : 'left') as 'left' | 'center' | 'right'
          })
        }
      } else if (tag === 'img') {
        const wp = el.getAttribute('data-width-percent')
        const al = el.getAttribute('data-align')
        const ap = el.getAttribute('data-asset-path') ?? ''
        roots.push({
          type: 'image',
          id,
          url: (el as HTMLImageElement).src,
          assetPath: ap || undefined,
          widthPercent: wp != null && wp !== '' ? parseInt(wp, 10) : undefined,
          align: (al === 'center' || al === 'right' ? al : 'left') as 'left' | 'center' | 'right'
        })
      } else if (el.classList.contains('file-block')) {
        const name = el.querySelector('.file-name')?.textContent ?? ''
        const fs = el.getAttribute('data-file-size')
        const ap = el.getAttribute('data-asset-path') ?? ''
        roots.push({ type: 'file', id, url: el.getAttribute('data-url') ?? '', fileName: name, fileSize: fs != null && fs !== '' ? parseInt(fs, 10) : undefined, assetPath: ap || undefined })
      } else if (tag === 'p' || tag === 'h1' || tag === 'h2') roots.push({ type: tag as 'p' | 'h1' | 'h2', id, children: collectChildren(el) })
      else if (tag === 'ul' || tag === 'ol') {
        const listChildren: EditorNode[] = []
        el.querySelectorAll(':scope > li').forEach(li => {
          listChildren.push({ type: 'li', id: (li as HTMLElement).getAttribute('data-id') ?? genId(), children: collectChildren(li as HTMLElement) })
        })
        if (listChildren.length === 0) listChildren.push({ type: 'li', id: genId(), children: [] })
        roots.push({ type: tag as 'ul' | 'ol', id, children: listChildren })
      } else if (tag === 'div') roots.push(...domToNodesFromContainer(el))
      else if (tag === 'br') roots.push({ type: 'p', id: genId(), children: [] })
      else {
        const text = el.textContent?.trim()
        if (text) roots.push({ type: 'p', id: genId(), children: [{ type: 'text', value: text }] })
      }
    }
  }
  return roots.length > 0 ? roots : [{ type: 'p', id: genId(), children: [] }]
}

const handleInput = () => {
  isInternalUpdate.value = true
  emit('update:modelValue', domToNodes())
  setTimeout(() => isInternalUpdate.value = false, 0)
}

const defaultNodes = (): EditorNode[] => [{ type: 'p', id: genId(), children: [] }]

onMounted(() => {
  if (!editorRef.value) return
  const nodes = Array.isArray(props.modelValue) && props.modelValue.length > 0 ? props.modelValue : defaultNodes()
  editorRef.value.innerHTML = nodesToHtml(nodes)
})

watch(() => props.modelValue, (newVal) => {
  if (!isInternalUpdate.value && editorRef.value) {
    const nodes = Array.isArray(newVal) && newVal.length > 0 ? newVal : defaultNodes()
    editorRef.value.innerHTML = nodesToHtml(nodes)
  }
}, { deep: true })

let savedRange: Range | null = null

const saveSelection = () => {
  const sel = window.getSelection()
  if (!editorRef.value || !sel || sel.rangeCount === 0) return
  const range = sel.getRangeAt(0)
  if (editorRef.value.contains(range.commonAncestorContainer)) {
    savedRange = range.cloneRange()
  } else {
    savedRange = null
  }
}

const restoreSelection = () => {
  if (!savedRange || !editorRef.value) return
  const sel = window.getSelection()
  if (!sel) return
  sel.removeAllRanges()
  sel.addRange(savedRange)
}

// 快捷命令执行（会先恢复工具栏点击前保存的选区）
const execCommand = (command: string, value?: string) => {
  restoreSelection()
  if (command === 'formatBlock' && value) {
    document.execCommand('formatBlock', false, value)
  } else if (command === 'justifyLeft' || command === 'justifyCenter' || command === 'justifyRight') {
    document.execCommand(command, false)
  } else {
    document.execCommand(command, false, value)
  }
  handleInput()
}

const selectedImageNode = computed(() => {
  if (!selectedImageId.value || !Array.isArray(props.modelValue)) return null
  return props.modelValue.find((n): n is EditorNode & { type: 'image' } => n.type === 'image' && (n as { id?: string }).id === selectedImageId.value) ?? null
})

function onEditorClick(e: MouseEvent) {
  const target = e.target as HTMLElement
  if (target.closest('.image-toolbar-root')) return
  if (target.classList.contains('image-block') || target.closest('.image-block-wrapper')) {
    const wrapper = target.closest('.image-block-wrapper') as HTMLElement | null
    const img = (wrapper?.querySelector('img.image-block') ?? target) as HTMLElement
    const id = img?.getAttribute('data-id') ?? null
    if (id) {
      selectedImageId.value = id
      const rect = img.getBoundingClientRect()
      let top: number
      if (rect.top - IMAGE_TOOLBAR_APPROX_HEIGHT - IMAGE_TOOLBAR_PADDING < 0) {
        top = rect.bottom + window.scrollY + IMAGE_TOOLBAR_PADDING
      } else {
        top = rect.top + window.scrollY - IMAGE_TOOLBAR_APPROX_HEIGHT - IMAGE_TOOLBAR_PADDING
      }
      let left = rect.left + rect.width / 2 - IMAGE_TOOLBAR_APPROX_WIDTH / 2
      left += window.scrollX
      left = Math.max(IMAGE_TOOLBAR_PADDING, Math.min(window.innerWidth - IMAGE_TOOLBAR_APPROX_WIDTH - IMAGE_TOOLBAR_PADDING + window.scrollX, left))
      selectedImageRect.value = { top, left }
    }
    return
  }
  selectedImageId.value = null
}

function onEditorContextMenu(e: MouseEvent) {
  const target = e.target as HTMLElement
  if (target.closest('.image-toolbar-root')) return
  if (target.classList.contains('image-block') || target.closest('.image-block-wrapper')) {
    const wrapper = target.closest('.image-block-wrapper') as HTMLElement | null
    const el = (wrapper ?? target) as HTMLElement
    const assetPath = el.getAttribute('data-asset-path') ?? ''
    const id = el.getAttribute('data-id') ?? ''
    if (assetPath) {
      e.preventDefault()
      emit('contextmenu', { type: 'image', assetPath, id, clientX: e.clientX, clientY: e.clientY })
    }
    return
  }
  if (target.classList.contains('file-block') || target.closest('.file-block')) {
    const el = target.closest('.file-block') as HTMLElement
    if (el) {
      const assetPath = el.getAttribute('data-asset-path') ?? ''
      const id = el.getAttribute('data-id') ?? ''
      if (assetPath) {
        e.preventDefault()
        emit('contextmenu', { type: 'file', assetPath, id, clientX: e.clientX, clientY: e.clientY })
      }
    }
  }
}

function updateImageNode(updates: { widthPercent?: number; align?: 'left' | 'center' | 'right' }) {
  if (!selectedImageId.value || !Array.isArray(props.modelValue)) return
  const next = props.modelValue.map(node => {
    if (node.type !== 'image' || node.id !== selectedImageId.value) return node
    return { ...node, ...updates }
  })
  emit('update:modelValue', next)
  // 直接同步 DOM，否则 watch 可能因 v-model 更新时机导致不重绘
  if (editorRef.value) editorRef.value.innerHTML = nodesToHtml(next)
}

function computeCursorBlockIndex(): number {
  const sel = window.getSelection()
  if (!sel || sel.rangeCount === 0 || !editorRef.value) return -1
  let node: Node | null = sel.anchorNode
  if (!node) return -1
  while (node && node.parentNode !== editorRef.value) node = node.parentNode
  if (!node) return -1
  const children = editorRef.value.childNodes
  for (let i = 0; i < children.length; i++) {
    if (children[i] === node) return i
  }
  return -1
}

/** 获取当前光标所在的根级块索引；若已失焦则返回上次记录的索引。 */
function getCursorBlockIndex(): number {
  const current = computeCursorBlockIndex()
  if (current >= 0) {
    lastCursorBlockIndex.value = current
    return current
  }
  return lastCursorBlockIndex.value
}

function selectionChangeHandler() {
  if (!editorRef.value) return
  const sel = window.getSelection()
  if (!sel || sel.rangeCount === 0) return
  const anchor = sel.anchorNode
  if (!anchor || !editorRef.value.contains(anchor)) return
  const idx = computeCursorBlockIndex()
  if (idx >= 0) lastCursorBlockIndex.value = idx
}
onMounted(() => {
  document.addEventListener('selectionchange', selectionChangeHandler)
})
onUnmounted(() => {
  document.removeEventListener('selectionchange', selectionChangeHandler)
})

defineExpose({ execCommand, handleInput, saveSelection, restoreSelection, getCursorBlockIndex })
</script>

<template>
  <div class="advanced-editor-container">
    <div
      ref="editorRef"
      class="advanced-editor"
      contenteditable="true"
      @input="handleInput"
      @keydown.enter="handleInput"
      @click="onEditorClick"
      @contextmenu="onEditorContextMenu"
    ></div>
    <Teleport to="body">
      <div
        v-if="selectedImageId && selectedImageNode"
        ref="imageToolbarRef"
        class="image-toolbar-root image-toolbar"
        :style="{ top: selectedImageRect.top + 'px', left: selectedImageRect.left + 'px' }"
      >
        <label class="image-toolbar-label">宽度%</label>
        <input
          type="number"
          class="image-toolbar-input"
          min="10"
          max="100"
          :value="selectedImageNode.widthPercent ?? 100"
          @change="(e) => updateImageNode({ widthPercent: Math.min(100, Math.max(10, Number((e.target as HTMLInputElement).value) || 100)) })"
        />
        <button type="button" class="image-toolbar-btn" title="左对齐" @click="updateImageNode({ align: 'left' })">
          <AlignLeft :size="16" />
        </button>
        <button type="button" class="image-toolbar-btn" title="居中" @click="updateImageNode({ align: 'center' })">
          <AlignCenter :size="16" />
        </button>
        <button type="button" class="image-toolbar-btn" title="右对齐" @click="updateImageNode({ align: 'right' })">
          <AlignRight :size="16" />
        </button>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.advanced-editor-container {
  width: 100%;
  height: 100%;
  background: var(--app-bg-color);
  color: var(--app-text-color);
}

.advanced-editor {
  outline: none;
  padding: 20px;
  min-height: 300px;
  line-height: 1.6;
  font-family: var(--app-font-family);
}

:deep(.editor-block) {
  margin-bottom: 0.5em;
}

:deep(h1.editor-block) {
  font-size: 2em;
  font-weight: bold;
  margin-top: 1em;
}

:deep(h2.editor-block) {
  font-size: 1.5em;
  font-weight: bold;
  margin-top: 0.8em;
}

:deep(.list-block) {
  margin: 0.5em 0;
  padding-left: 1.5em;
}

:deep(.list-block li) {
  margin: 0.2em 0;
}

:deep(.image-block) {
  max-width: 100%;
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0,0,0,0.1);
  margin: 5px;
  cursor: default;
  display: inline-block;
  vertical-align: middle;
}

:deep(.file-block) {
  margin: 0 5px;
  cursor: default;
  display: inline-block;
  vertical-align: middle;
}

:deep(.file-card) {
  display: inline-flex;
  align-items: center;
  padding: 12px 20px;
  background: var(--file-card-bg);
  border-radius: 8px;
  border: 1px solid var(--file-card-border);
  gap: 10px;
  color: var(--app-text-color);
}

:deep(.file-icon) {
  font-size: 20px;
}

:deep(.file-name) {
  font-size: 14px;
  color: var(--app-text-color);
}

:deep(.file-size) {
  font-size: 12px;
  color: var(--file-size-color);
  margin-left: 6px;
}

.image-toolbar {
  position: absolute;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 8px;
  background: var(--app-bg-color);
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  border: 1px solid rgba(0, 0, 0, 0.1);
  z-index: 1000;
}

.dark .image-toolbar {
  border-color: rgba(255, 255, 255, 0.1);
  background: rgba(30, 30, 30, 0.95);
}

.image-toolbar-label {
  font-size: 12px;
  color: var(--app-text-color);
  white-space: nowrap;
}

.image-toolbar-input {
  width: 52px;
  padding: 4px 6px;
  font-size: 12px;
  font-family: var(--app-font-family);
  color: var(--app-text-color);
  background: rgba(0, 0, 0, 0.06);
  border: 1px solid rgba(0, 0, 0, 0.1);
  border-radius: 4px;
}

.dark .image-toolbar-input {
  background: rgba(255, 255, 255, 0.08);
  border-color: rgba(255, 255, 255, 0.15);
}

.image-toolbar-btn {
  width: 28px;
  height: 28px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  color: var(--app-text-color);
  background: transparent;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.2s;
}

.image-toolbar-btn:hover {
  background: rgba(0, 0, 0, 0.08);
}

.dark .image-toolbar-btn:hover {
  background: rgba(255, 255, 255, 0.1);
}
</style>
