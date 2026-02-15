<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'

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
  | { type: 'image'; id: string; url: string }
  | { type: 'file'; id: string; url: string; fileName?: string }

const props = defineProps<{
  modelValue: EditorNode[]
}>()

const emit = defineEmits(['update:modelValue', 'upload-image', 'upload-file'])

const editorRef = ref<HTMLDivElement | null>(null)
const isInternalUpdate = ref(false)

const genId = () => crypto.randomUUID()

const escapeHtml = (text: string) => {
  const div = document.createElement('div')
  div.textContent = text
  return div.innerHTML
}

// 树形节点转 HTML
const nodeToHtml = (node: EditorNode): string => {
  if (node.type === 'text') return escapeHtml(node.value)
  if (node.type === 'image') {
    return `<img class="editor-block image-block" data-id="${node.id}" src="${node.url}" style="max-width: 100%; display: inline-block; vertical-align: middle; margin: 5px;" onerror="console.error('Image load failed:', this.src)" />`
  }
  if (node.type === 'file') {
    const name = escapeHtml(node.fileName ?? '')
    const url = node.url ?? ''
    return `<span class="editor-block file-block" data-id="${node.id}" data-url="${escapeHtml(url)}" contenteditable="false" style="display: inline-block; vertical-align: middle; margin: 5px;">
      <span class="file-card"><span class="file-icon">📄</span><span class="file-name">${name}</span></span>
    </span>`
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

    if (tag === 'img') {
      out.push({ type: 'image', id, url: (child as HTMLImageElement).src })
    } else if (child.classList.contains('file-block')) {
      const name = child.querySelector('.file-name')?.textContent ?? ''
      const url = child.getAttribute('data-url') ?? ''
      out.push({ type: 'file', id, url, fileName: name })
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

    if (tag === 'img') {
      roots.push({ type: 'image', id, url: (child as HTMLImageElement).src })
    } else if (child.classList.contains('file-block')) {
      const name = child.querySelector('.file-name')?.textContent ?? ''
      const url = child.getAttribute('data-url') ?? ''
      roots.push({ type: 'file', id, url, fileName: name })
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
      if (tag === 'img') roots.push({ type: 'image', id, url: (el as HTMLImageElement).src })
      else if (el.classList.contains('file-block')) {
        const name = el.querySelector('.file-name')?.textContent ?? ''
        roots.push({ type: 'file', id, url: el.getAttribute('data-url') ?? '', fileName: name })
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
  } else {
    document.execCommand(command, false, value)
  }
  handleInput()
}

defineExpose({ execCommand, handleInput, saveSelection, restoreSelection })
</script>

<template>
  <div class="advanced-editor-container">
    <div
      ref="editorRef"
      class="advanced-editor"
      contenteditable="true"
      @input="handleInput"
      @keydown.enter="handleInput"
    ></div>
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
  margin: 5px;
  cursor: default;
  display: inline-block;
  vertical-align: middle;
}

:deep(.file-card) {
  display: inline-flex;
  align-items: center;
  padding: 12px 20px;
  background: rgba(0,0,0,0.05);
  border-radius: 8px;
  border: 1px solid rgba(0,0,0,0.1);
  gap: 10px;
}

.dark :deep(.file-card) {
  background: rgba(255,255,255,0.05);
  border-color: rgba(255,255,255,0.1);
}

:deep(.file-icon) {
  font-size: 20px;
}

:deep(.file-name) {
  font-size: 14px;
}
</style>
