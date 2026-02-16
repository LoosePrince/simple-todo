<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { ElMessage } from 'element-plus';
import hljs from 'highlight.js';
import 'highlight.js/styles/github.css';
import { AlignCenter, AlignLeft, AlignRight, Copy } from 'lucide-vue-next';
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';

/** 树形节点：类似 HTML 序列化为 JSON，支持子节点，加粗/斜体等内联格式保存在子节点中 */
export type EditorNode =
  | { type: 'text'; value: string }
  | { type: 'p'; id?: string; align?: 'left' | 'center' | 'right'; children: EditorNode[] }
  | { type: 'h1'; id?: string; align?: 'left' | 'center' | 'right'; children: EditorNode[] }
  | { type: 'h2'; id?: string; align?: 'left' | 'center' | 'right'; children: EditorNode[] }
  | { type: 'strong'; children: EditorNode[] }
  | { type: 'em'; children: EditorNode[] }
  | { type: 'color'; color: string; children: EditorNode[] }
  | { type: 'ul'; id?: string; children: EditorNode[] }
  | { type: 'ol'; id?: string; children: EditorNode[] }
  | { type: 'li'; id?: string; children: EditorNode[] }
  | { type: 'taskList'; id?: string; children: EditorNode[] }
  | { type: 'taskItem'; id?: string; checked: boolean; children: EditorNode[] }
  | { type: 'image'; id: string; url: string; assetPath?: string; widthPercent?: number; align?: 'left' | 'center' | 'right' }
  | { type: 'file'; id: string; url: string; fileName?: string; fileSize?: number; assetPath?: string; align?: 'left' | 'center' | 'right' }
  | { type: 'code'; id: string; content: string; language?: string }
  | { type: 'fold'; id: string; folded: boolean; children: EditorNode[] }

const props = defineProps<{
  modelValue: EditorNode[]
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: EditorNode[]): void
  (e: 'upload-image'): void
  (e: 'upload-file'): void
  (e: 'contextmenu', payload: { type: 'image' | 'file'; assetPath: string; id: string; clientX: number; clientY: number }): void
  (e: 'open-asset', payload: { type: 'image' | 'file'; assetPath: string; id: string }): void
  (e: 'paste-files', payload: { files: File[]; text: string }): void
}>()

const editorRef = ref<HTMLDivElement | null>(null)
const isInternalUpdate = ref(false)
/** 选区在编辑器内时更新的“光标所在块”索引，用于失焦后仍能在该位置插入 */
const lastCursorBlockIndex = ref(-1)
const selectedImageId = ref<string | null>(null)
const selectedImageRect = ref({ top: 0, left: 0 })
const imageToolbarRef = ref<HTMLElement | null>(null)
const selectedFileId = ref<string | null>(null)
const selectedFileRect = ref({ top: 0, left: 0 })
const fileToolbarRef = ref<HTMLElement | null>(null)
const IMAGE_TOOLBAR_APPROX_HEIGHT = 44
const IMAGE_TOOLBAR_APPROX_WIDTH = 220
const FILE_TOOLBAR_APPROX_HEIGHT = 44
const FILE_TOOLBAR_APPROX_WIDTH = 180
const IMAGE_TOOLBAR_PADDING = 8

const { t } = useI18n()
const foldTitleText = computed(() => t('editor.foldTitle'))
const foldInsertAfterText = computed(() => t('editor.foldInsertAfter'))

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
    return `<span class="image-block-wrapper editor-block" data-id="${node.id}" data-align="${align}" data-width-percent="${w}" data-asset-path="${escapeHtml(ap)}" style="display: block; text-align: ${align}; margin: 8px 0;"><img class="image-block" data-id="${node.id}" data-width-percent="${w}" data-align="${align}" data-asset-path="${escapeHtml(ap)}" src="${node.url}" style="width: ${w}%; max-width: 100%; display: inline-block; vertical-align: middle; border-radius: 8px; cursor: pointer;" contenteditable="false" onerror="console.error('Image load failed:', this.src)" /><br class="asset-trailing-br"></span>`
  }
  if (node.type === 'file') {
    const name = escapeHtml(node.fileName ?? '')
    const url = node.url ?? ''
    const sizeStr = node.fileSize != null ? formatFileSize(node.fileSize) : ''
    const ap = node.assetPath ?? ''
    const align = node.align ?? 'left'
    const ext = (node.fileName ?? '').split('.').pop()?.toLowerCase().replace(/[^a-z0-9]/g, '') || ''
    const extAttr = ext ? ` data-ext="${escapeHtml(ext)}"` : ''
    const alignStyle = align !== 'left' ? `text-align: ${align};` : ''
    const alignData = align !== 'left' ? ` data-align="${align}"` : ''
    return `<span class="editor-block file-block-wrapper" data-id="${node.id}" data-url="${escapeHtml(url)}" data-asset-path="${escapeHtml(ap)}"${alignData} style="display: block; ${alignStyle} margin: 8px 0;"><span class="file-block" style="display: inline-block; vertical-align: middle; margin: 0 5px;"><span class="file-card" contenteditable="false"><span class="file-icon"${extAttr}><span class="file-icon-fallback">📄</span><img class="file-icon-img" alt=""></span><span class="file-name">${name}</span>${sizeStr ? `<span class="file-size">${escapeHtml(sizeStr)}</span>` : ''}</span></span><br class="asset-trailing-br"></span>`
  }
  if (node.type === 'code') {
    const lang = node.language || 'text'
    const insertLabel = escapeHtml(foldInsertAfterText.value)
    const copyLabel = escapeHtml(t('editor.codeCopy'))
    return `<div class="editor-block code-block-wrapper" data-id="${node.id}" data-language="${escapeHtml(lang)}" contenteditable="false" style="margin: 12px 0;"><div class="code-card"><div class="code-header"><span class="code-lang">${escapeHtml(lang)}</span><div class="code-header-btns"><button type="button" class="fold-copy-btn" title="${copyLabel}"><span class="copy-icon-placeholder"></span></button><button type="button" class="fold-insert-btn" title="${insertLabel}">＋</button></div></div><div class="code-editor" contenteditable="true" spellcheck="false">${escapeHtml(node.content)}</div></div></div>`
  }
  if (node.type === 'fold') {
    const folded = node.folded ?? false
    const title = escapeHtml(foldTitleText.value)
    const insertLabel = escapeHtml(foldInsertAfterText.value)
    return `<div class="editor-block fold-block" data-id="${node.id}" data-folded="${folded}" contenteditable="false" style="margin: 20px 0;"><div class="fold-line-top"></div><div class="fold-header"><button type="button" class="fold-toggle-btn">${folded ? '▶' : '▼'}</button><span class="fold-title">${title}</span><button type="button" class="fold-insert-btn" title="${insertLabel}">＋</button></div><div class="fold-content" contenteditable="true" style="display: ${folded ? 'none' : 'block'};">${(node.children || []).map(nodeToHtml).join('') || '<p class="editor-block"><br></p>'}</div><div class="fold-line-bottom"></div></div>`
  }
  if (node.type === 'strong') return `<strong>${(node.children || []).map(nodeToHtml).join('')}</strong>`
  if (node.type === 'em') return `<em>${(node.children || []).map(nodeToHtml).join('')}</em>`
  if (node.type === 'color') {
    const color = node.color && /^#([0-9A-Fa-f]{3}){1,2}$/.test(node.color) ? node.color : (node.color || '#000000')
    return `<span style="color: ${escapeHtml(color)}">${(node.children || []).map(nodeToHtml).join('')}</span>`
  }
  if (node.type === 'li') {
    const inner = (node.children || []).map(nodeToHtml).join('') || '<br>'
    return `<li class="editor-block" data-id="${node.id ?? genId()}">${inner}</li>`
  }
  if (node.type === 'ul' || node.type === 'ol') {
    const inner = (node.children || []).map(nodeToHtml).join('') || '<li><br></li>'
    return `<${node.type} class="editor-block list-block" data-id="${node.id ?? genId()}">${inner}</${node.type}>`
  }
  if (node.type === 'taskList') {
    const inner = (node.children || []).map(nodeToHtml).join('') || ''
    return `<ul class="editor-block task-list" data-id="${node.id ?? genId()}" data-type="task">${inner || '<li class="editor-block task-item" data-id="' + genId() + '" data-checked="false"><input type="checkbox" class="task-item-checkbox" contenteditable="false"><span class="task-item-content"><br></span></li>'}</ul>`
  }
  if (node.type === 'taskItem') {
    const checked = node.checked ?? false
    const inner = (node.children || []).map(nodeToHtml).join('') || '<br>'
    return `<li class="editor-block task-item" data-id="${node.id ?? genId()}" data-checked="${checked}"><input type="checkbox" class="task-item-checkbox" contenteditable="false"${checked ? ' checked' : ''}><span class="task-item-content">${inner}</span></li>`
  }
  if (node.type === 'p' || node.type === 'h1' || node.type === 'h2') {
    const inner = (node.children || []).map(nodeToHtml).join('') || '<br>'
    const align = (node as { align?: string }).align
    const alignStyle = align && align !== 'left' ? ` style="text-align: ${align}"` : ''
    const alignData = align ? ` data-align="${align}"` : ''
    return `<${node.type} class="editor-block" data-id="${node.id ?? genId()}"${alignData}${alignStyle}>${inner}</${node.type}>`
  }
  return ''
}

const nodesToHtml = (nodes: EditorNode[]) => nodes.map(nodeToHtml).join('')

function getBlockAlign(el: HTMLElement): 'left' | 'center' | 'right' {
  const raw = el.style?.textAlign || el.getAttribute('data-align') || (typeof getComputedStyle !== 'undefined' ? getComputedStyle(el).textAlign : '')
  if (raw === 'center') return 'center'
  if (raw === 'right') return 'right'
  return 'left'
}

/** 从 span 元素解析 color 样式，返回 hex 或空 */
function getSpanColor(el: HTMLElement): string | null {
  const color = el.style?.color || (typeof getComputedStyle !== 'undefined' ? getComputedStyle(el).color : '')
  if (!color || color === 'rgba(0, 0, 0, 0)' || color === 'transparent') return null
  if (color.startsWith('#')) return color
  const rgbMatch = color.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/)
  if (rgbMatch) {
    const r = parseInt(rgbMatch[1]).toString(16).padStart(2, '0')
    const g = parseInt(rgbMatch[2]).toString(16).padStart(2, '0')
    const b = parseInt(rgbMatch[3]).toString(16).padStart(2, '0')
    return `#${r}${g}${b}`
  }
  return null
}

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
      // file-block 可能在 file-block-wrapper 内部，需要向上查找 wrapper 获取 assetPath
      const wrapper = child.closest('.file-block-wrapper') as HTMLElement | null
      const name = child.querySelector('.file-name')?.textContent ?? ''
      const url = wrapper?.getAttribute('data-url') ?? child.getAttribute('data-url') ?? ''
      const ap = wrapper?.getAttribute('data-asset-path') ?? child.getAttribute('data-asset-path') ?? ''
      const align = wrapper ? getBlockAlign(wrapper) : 'left'
      out.push({ type: 'file', id, url, fileName: name, assetPath: ap || undefined, ...(align !== 'left' ? { align } : {}) })
    } else if (child.classList.contains('file-block-wrapper')) {
      const fileBlock = child.querySelector('.file-block') as HTMLElement | null
      const name = fileBlock?.querySelector('.file-name')?.textContent ?? ''
      const url = child.getAttribute('data-url') ?? ''
      const ap = child.getAttribute('data-asset-path') ?? ''
      const align = getBlockAlign(child)
      out.push({ type: 'file', id, url, fileName: name, assetPath: ap || undefined, ...(align !== 'left' ? { align } : {}) })
    } else if (child.classList.contains('code-block-wrapper')) {
      const codeEditor = child.querySelector('.code-editor')
      const content = codeEditor?.textContent ?? ''
      const lang = child.getAttribute('data-language') ?? 'text'
      out.push({ type: 'code', id, content, language: lang })
    } else if (child.classList.contains('fold-block')) {
      const folded = child.getAttribute('data-folded') === 'true'
      const contentEl = child.querySelector('.fold-content') as HTMLElement | null
      const children = contentEl ? domToNodesFromContainer(contentEl) : []
      out.push({ type: 'fold', id, folded, children })
    } else if (child.classList.contains('task-item-checkbox')) {
      // skip checkbox, it is reflected in data-checked on the li
    } else if (child.classList.contains('task-item-content')) {
      out.push(...collectChildren(child))
    } else if (tag === 'strong' || tag === 'b') {
      out.push({ type: 'strong', children: collectChildren(child) })
    } else if (tag === 'em' || tag === 'i') {
      out.push({ type: 'em', children: collectChildren(child) })
    } else if (tag === 'span' && getSpanColor(child)) {
      out.push({ type: 'color', color: getSpanColor(child)!, children: collectChildren(child) })
    } else if (tag === 'p' || tag === 'h1' || tag === 'h2') {
      const align = getBlockAlign(child)
      out.push({ type: tag as 'p' | 'h1' | 'h2', id, ...(align !== 'left' ? { align } : {}), children: collectChildren(child) })
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
    } else if (tag === 'font') {
      const fontColor = child.getAttribute('color')
      if (fontColor) {
        const hex = fontColor.startsWith('#') ? fontColor : `#${fontColor}`
        out.push({ type: 'color', color: hex, children: collectChildren(child) })
      } else {
        out.push(...collectChildren(child))
      }
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
  return domToNodesFromContainer(editorRef.value)
}

function domToNodesFromContainer(container: HTMLElement): EditorNode[] {
  const roots: EditorNode[] = []
  for (const node of container.childNodes) {
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
        roots.push(...collectTrailingBlocksFromWrapper(child, img))
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
    } else if (child.classList.contains('file-block-wrapper')) {
      const fileBlock = child.querySelector('.file-block') as HTMLElement | null
      const fileCard = fileBlock?.querySelector('.file-card') as HTMLElement | null
      const name = fileBlock?.querySelector('.file-name')?.textContent ?? ''
      const url = child.getAttribute('data-url') ?? ''
      const ap = child.getAttribute('data-asset-path') ?? ''
      const align = getBlockAlign(child)
      roots.push({ type: 'file', id, url, fileName: name, assetPath: ap || undefined, ...(align !== 'left' ? { align } : {}) })
      if (fileCard) {
        roots.push(...collectTrailingBlocksFromFileBlock(child, fileCard))
      }
    } else if (child.classList.contains('file-block')) {
      // file-block 可能在 file-block-wrapper 内部，需要向上查找 wrapper 获取 assetPath
      const wrapper = child.closest('.file-block-wrapper') as HTMLElement | null
      const fileCard = child.querySelector('.file-card')
      const name = child.querySelector('.file-name')?.textContent ?? ''
      const url = wrapper?.getAttribute('data-url') ?? child.getAttribute('data-url') ?? ''
      const ap = wrapper?.getAttribute('data-asset-path') ?? child.getAttribute('data-asset-path') ?? ''
      const align = wrapper ? getBlockAlign(wrapper) : 'left'
      roots.push({ type: 'file', id, url, fileName: name, assetPath: ap || undefined, ...(align !== 'left' ? { align } : {}) })
      if (fileCard) {
        roots.push(...collectTrailingBlocksFromFileBlock(child, fileCard))
      }
    } else if (child.classList.contains('code-block-wrapper')) {
      const codeEditor = child.querySelector('.code-editor')
      const content = codeEditor?.textContent ?? ''
      const lang = child.getAttribute('data-language') ?? 'text'
      roots.push({ type: 'code', id, content, language: lang })
    } else if (child.classList.contains('fold-block')) {
      const folded = child.getAttribute('data-folded') === 'true'
      const contentEl = child.querySelector('.fold-content') as HTMLElement | null
      const children = contentEl ? domToNodesFromContainer(contentEl) : []
      roots.push({ type: 'fold', id, folded, children })
    } else if (tag === 'p' || tag === 'h1' || tag === 'h2') {
      const align = getBlockAlign(child)
      roots.push({ type: tag as 'p' | 'h1' | 'h2', id, ...(align !== 'left' ? { align } : {}), children: collectChildren(child) })
    } else if (child.classList.contains('task-list')) {
      const taskListChildren: EditorNode[] = []
      child.querySelectorAll(':scope > li.task-item').forEach(liEl => {
        const li = liEl as HTMLElement
        const contentEl = li.querySelector('.task-item-content') as HTMLElement | null
        const children = contentEl ? collectChildren(contentEl) : []
        const checked = li.getAttribute('data-checked') === 'true' || (li.querySelector('.task-item-checkbox') as HTMLInputElement | null)?.checked === true
        taskListChildren.push({ type: 'taskItem', id: li.getAttribute('data-id') ?? genId(), checked, children })
      })
      if (taskListChildren.length === 0) taskListChildren.push({ type: 'taskItem', id: genId(), checked: false, children: [] })
      roots.push({ type: 'taskList', id: child.getAttribute('data-id') ?? genId(), children: taskListChildren })
    } else if (tag === 'ul' || tag === 'ol') {
      const listChildren: EditorNode[] = []
      child.querySelectorAll(':scope > li').forEach(li => {
        listChildren.push({ type: 'li', id: (li as HTMLElement).getAttribute('data-id') ?? genId(), children: collectChildren(li as HTMLElement) })
      })
      if (listChildren.length === 0) listChildren.push({ type: 'li', id: genId(), children: [] })
      roots.push({ type: tag as 'ul' | 'ol', id, children: listChildren })
    } else if (tag === 'div') {
      const fromDiv = domToNodesFromContainer(child)
      roots.push(...fromDiv)
    } else if (tag === 'br') {
      roots.push({ type: 'p', id: genId(), children: [] })
    } else {
      const text = child.textContent?.trim()
      if (text) roots.push({ type: 'p', id: genId(), children: [{ type: 'text', value: text }] })
    }
  }
  return roots.length > 0 ? roots : [{ type: 'p', id: genId(), children: [] }]
}

/** 收集图片 wrapper 内 img 之后的兄弟节点并序列化为块（解决在图片后直接输入时内容被丢的问题） */
function collectTrailingBlocksFromWrapper(wrapper: HTMLElement, img: Element): EditorNode[] {
  const temp = document.createElement('div')
  let next: ChildNode | null = img.nextSibling
  while (next) {
    if (next.nodeType === Node.ELEMENT_NODE && (next as HTMLElement).classList.contains('asset-trailing-br')) {
      next = next.nextSibling
      continue
    }
    temp.appendChild(next.cloneNode(true))
    next = next.nextSibling
  }
  if (temp.childNodes.length === 0) return []
  return domToNodesFromContainer(temp)
}

/** 收集文件块内 file-card 之后的兄弟节点并序列化为块 */
function collectTrailingBlocksFromFileBlock(wrapper: HTMLElement, fileCard: Element): EditorNode[] {
  const temp = document.createElement('div')
  let next: ChildNode | null = fileCard.nextSibling
  while (next) {
    if (next.nodeType === Node.ELEMENT_NODE && (next as HTMLElement).classList.contains('asset-trailing-br')) {
      next = next.nextSibling
      continue
    }
    temp.appendChild(next.cloneNode(true))
    next = next.nextSibling
  }
  if (temp.childNodes.length === 0) return []
  return domToNodesFromContainer(temp)
}

const handleInput = () => {
  isInternalUpdate.value = true
  emit('update:modelValue', domToNodes())
  setTimeout(() => isInternalUpdate.value = false, 0)
}

/** 在当前位置插入纯文本，并触发同步 */
function insertPlainText(text: string) {
  if (!text) return
  const normalized = text.replace(/\r\n/g, '\n')
  document.execCommand('insertText', false, normalized)
  handleInput()
}

/** 粘贴：若包含文件则交给上层处理，否则当作纯文本 */
function handlePaste(e: ClipboardEvent) {
  const data = e.clipboardData
  if (!data) return

  const items = Array.from(data.items || [])
  const files: File[] = []
  for (const item of items) {
    if (item.kind === 'file') {
      const f = item.getAsFile()
      if (f) files.push(f)
    }
  }

  const text = data.getData('text/plain') ?? ''

  if (files.length > 0) {
    e.preventDefault()
    emit('paste-files', { files, text })
    return
  }

  e.preventDefault()
  if (!text) return
  insertPlainText(text)
}

/** 获取当前光标所在的根级块索引及其容器 */
function getInsertionContainer(): { container: HTMLElement | null, index: number } {
  const sel = window.getSelection()
  const editor = editorRef.value
  if (!sel || sel.rangeCount === 0 || !editor) return { container: editor, index: -1 }
  
  const anchorNode = sel.anchorNode
  if (!anchorNode) return { container: editor, index: -1 }

  // 寻找最近的 contenteditable="true" 容器
  let container: HTMLElement | null = null
  let curr: Node | null = anchorNode
  while (curr && curr !== editor) {
    if (curr instanceof HTMLElement && curr.getAttribute('contenteditable') === 'true') {
      container = curr
      break
    }
    curr = curr.parentNode
  }
  
  if (!container) container = editor

  // 寻找容器内的根级块
  let node: Node | null = anchorNode
  while (node && node.parentNode !== container) node = node.parentNode
  
  if (!node) return { container, index: -1 }
  
  const kids = container.childNodes
  for (let i = 0; i < kids.length; i++) {
    if (kids[i] === node) return { container, index: i }
  }
  
  return { container, index: -1 }
}

/** 阻止在资产块内部（非编辑器区域）输入 */
function handleBeforeInput(e: Event) {
  const inputEvent = e as InputEvent
  const selection = window.getSelection()
  if (!selection || selection.rangeCount === 0) return
  
  const range = selection.getRangeAt(0)
  const container = range.commonAncestorContainer
  const containerEl = container.nodeType === Node.ELEMENT_NODE ? container as HTMLElement : container.parentElement as HTMLElement | null
  
  const innerEditable = containerEl?.closest('.code-editor, .fold-content')
  
  // 处理退格键：如果光标在资产块后的第一个字符，按退格键应该选中该资产块
  if (inputEvent.inputType === 'deleteContentBackward' && range.collapsed && range.startOffset === 0 && !innerEditable) {
    const info = getInsertionContainer()
    const block = (info.index >= 0 && info.container) ? info.container.childNodes[info.index] : null
    
    if (block) {
      const prev = block.previousSibling as HTMLElement | null
      if (prev && (prev.classList.contains('image-block-wrapper') || 
                   prev.classList.contains('file-block-wrapper') || 
                   prev.classList.contains('code-block-wrapper') || 
                   prev.classList.contains('fold-block'))) {
        e.preventDefault()
        const newRange = document.createRange()
        newRange.selectNode(prev)
        selection.removeAllRanges()
        selection.addRange(newRange)
        return
      }
    }
  }

  if (innerEditable) return

  const wrapper = containerEl?.closest('.image-block-wrapper, .file-block-wrapper, .file-block, .code-block-wrapper, .fold-block') as HTMLElement | null
  
  if (!wrapper) return
  
  const img = wrapper.querySelector('img.image-block')
  const fileCard = wrapper.querySelector('.file-card')
  const codeCard = wrapper.querySelector('.code-card')
  const isFold = wrapper.classList.contains('fold-block')
  const trailingBr = wrapper.querySelector('.asset-trailing-br')
  
  const isInAssetStructure = img?.contains(container) || 
                             fileCard?.contains(container) || 
                             (codeCard?.contains(container) && !containerEl?.closest('.code-editor')) ||
                             (isFold && !containerEl?.closest('.fold-content'))
  
  if (isInAssetStructure) {
    e.preventDefault()
    return
  }
  
  const isAtTrailingBrOrEnd = (trailingBr && (container === trailingBr || container.parentElement === trailingBr.parentElement)) ||
    (container === wrapper && range.startOffset >= (wrapper.childNodes.length || 1))
  if (isAtTrailingBrOrEnd) {
    if (inputEvent.inputType === 'insertLineBreak' || (inputEvent.inputType === 'insertText' && inputEvent.data === '\n')) {
      return
    }
    e.preventDefault()
    const wrapperParent = wrapper.parentElement
    if (!wrapperParent) return
    const newP = document.createElement('p')
    newP.className = 'editor-block'
    newP.setAttribute('data-id', genId())
    const br = document.createElement('br')
    newP.appendChild(br)
    if (wrapper.nextSibling) {
      wrapperParent.insertBefore(newP, wrapper.nextSibling)
    } else {
      wrapperParent.appendChild(newP)
    }
    const newRange = document.createRange()
    newRange.setStart(newP, 0)
    newRange.collapse(true)
    selection.removeAllRanges()
    selection.addRange(newRange)
    if (inputEvent.inputType === 'insertText' && inputEvent.data) {
      document.execCommand('insertText', false, inputEvent.data)
    }
    handleInput()
    return
  }
  
  if (wrapper.contains(container) && !isInAssetStructure && container !== trailingBr && container.parentElement !== trailingBr?.parentElement) {
    e.preventDefault()
    const nextSibling = wrapper.nextSibling
    if (nextSibling) {
      const newRange = document.createRange()
      if (nextSibling.nodeType === Node.ELEMENT_NODE) {
        newRange.setStart(nextSibling as Node, 0)
      } else {
        newRange.setStartAfter(wrapper)
      }
      newRange.collapse(true)
      selection.removeAllRanges()
      selection.addRange(newRange)
    }
  }
}

/** 处理 Enter 键：在资产后面时插入新段落 */
function handleEnterKey(e: KeyboardEvent) {
  const selection = window.getSelection()
  if (!selection || selection.rangeCount === 0 || !editorRef.value) return
  
  const range = selection.getRangeAt(0)
  const container = range.commonAncestorContainer
  const containerEl = container.nodeType === Node.ELEMENT_NODE ? container as HTMLElement : container.parentElement as HTMLElement | null
  
  if (containerEl?.closest('.code-editor, .fold-content')) {
    return
  }

  const wrapper = containerEl?.closest('.image-block-wrapper, .file-block-wrapper, .file-block, .code-block-wrapper, .fold-block') as HTMLElement | null
  if (!wrapper) return
  
  const img = wrapper.querySelector('img.image-block')
  const fileCard = wrapper.querySelector('.file-card')
  const codeCard = wrapper.querySelector('.code-card')
  const isFold = wrapper.classList.contains('fold-block')
  const trailingBr = wrapper.querySelector('.asset-trailing-br')
  
  const isInAssetStructure = img?.contains(container) || 
                             fileCard?.contains(container) || 
                             (codeCard?.contains(container) && !containerEl?.closest('.code-editor')) ||
                             (isFold && !containerEl?.closest('.fold-content'))
  if (isInAssetStructure) return
  
  const isInsideWrapper = wrapper === container || wrapper.contains(container)
  const isAtEndOfWrapper = container === wrapper && range.startOffset >= wrapper.childNodes.length
  const isAtTrailingBr = trailingBr && (container === trailingBr || container.parentElement === trailingBr.parentElement || range.startContainer === trailingBr)
  const isAfterAsset = isInsideWrapper && (isAtTrailingBr || isAtEndOfWrapper || (container === wrapper && range.startOffset > 0))
  
  const wrapperParent = wrapper.parentElement
  const containerParent = container.parentElement
  const isRightAfterWrapper = wrapperParent && containerParent && wrapperParent === containerParent && 
    container.nodeType === Node.TEXT_NODE && 
    Array.from(wrapperParent.childNodes).indexOf(wrapper) + 1 === Array.from(wrapperParent.childNodes).indexOf(container as ChildNode)
  
  if (isAfterAsset || isRightAfterWrapper) {
    e.preventDefault()
    const newP = document.createElement('p')
    newP.className = 'editor-block'
    newP.setAttribute('data-id', genId())
    const br = document.createElement('br')
    newP.appendChild(br)
    
    if (wrapperParent) {
      if (wrapper.nextSibling) {
        wrapperParent.insertBefore(newP, wrapper.nextSibling)
      } else {
        wrapperParent.appendChild(newP)
      }
    }
    
    const newRange = document.createRange()
    newRange.setStart(newP, 0)
    newRange.collapse(true)
    selection.removeAllRanges()
    selection.addRange(newRange)
    handleInput()
  }
}

function handleKeyDown(e: KeyboardEvent) {
  const target = e.target as HTMLElement
  const isInsideCode = target.classList.contains('code-editor') || target.closest('.code-editor')
  
  // 处理代码块内的全选 (Ctrl+A)
  if (isInsideCode && e.ctrlKey && e.key === 'a') {
    e.preventDefault()
    const editor = target.classList.contains('code-editor') ? target : target.closest('.code-editor') as HTMLElement
    const selection = window.getSelection()
    if (selection && editor) {
      const range = document.createRange()
      range.selectNodeContents(editor)
      selection.removeAllRanges()
      selection.addRange(range)
    }
    return
  }

  if (e.key === 'ArrowDown') {
    const selection = window.getSelection()
    if (!selection || selection.rangeCount === 0 || !editorRef.value) return
    const range = selection.getRangeAt(0)
    const container = range.commonAncestorContainer
    
    const foldContent = container instanceof HTMLElement ? container.closest('.fold-content') : container.parentElement?.closest('.fold-content')
    if (foldContent) {
      const lastRange = document.createRange()
      lastRange.selectNodeContents(foldContent)
      lastRange.collapse(false)
      
      const isAtEnd = range.compareBoundaryPoints(Range.END_TO_END, lastRange) >= 0
      
      if (isAtEnd) {
        const foldBlock = foldContent.closest('.fold-block') as HTMLElement | null
        if (foldBlock) {
          e.preventDefault()
          const nextSibling = foldBlock.nextSibling
          if (nextSibling && nextSibling.nodeType === Node.ELEMENT_NODE && (nextSibling as HTMLElement).tagName.toLowerCase() === 'p') {
            const newRange = document.createRange()
            newRange.setStart(nextSibling, 0)
            newRange.collapse(true)
            selection.removeAllRanges()
            selection.addRange(newRange)
          } else {
            const newP = document.createElement('p')
            newP.className = 'editor-block'
            newP.setAttribute('data-id', genId())
            newP.innerHTML = '<br>'
            foldBlock.parentNode?.insertBefore(newP, foldBlock.nextSibling)
            const newRange = document.createRange()
            newRange.setStart(newP, 0)
            newRange.collapse(true)
            selection.removeAllRanges()
            selection.addRange(newRange)
            handleInput()
          }
        }
      }
    }
  }
}

const defaultNodes = (): EditorNode[] => [{ type: 'p', id: genId(), children: [] }]

async function updateFileIcons() {
  const el = editorRef.value
  if (!el) return
  const icons = el.querySelectorAll<HTMLElement>('.file-icon[data-ext]')
  const exts = new Set<string>()
  icons.forEach(node => {
    const ext = node.getAttribute('data-ext')
    if (ext) exts.add(ext)
  })
  const cache = new Map<string, string>()
  for (const ext of exts) {
    try {
      const b64 = await invoke<string>('get_file_icon', { extension: ext })
      if (b64) cache.set(ext, b64)
    } catch (_) {}
  }
  icons.forEach(node => {
    const ext = node.getAttribute('data-ext')
    const img = node.querySelector<HTMLImageElement>('.file-icon-img')
    if (!ext || !img) return
    const dataUrl = cache.get(ext)
    if (dataUrl) {
      img.src = `data:image/png;base64,${dataUrl}`
      img.style.display = ''
    }
  })
}

/** 渲染静态插入的 Lucide 图标 */
function renderStaticIcons() {
  const el = editorRef.value
  if (!el) return
  const placeholders = el.querySelectorAll('.copy-icon-placeholder')
  placeholders.forEach(p => {
    p.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-copy"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/></svg>`
  })
}

function updateCodeHighlighting() {
  const el = editorRef.value
  if (!el) return
  const codeEditors = el.querySelectorAll<HTMLElement>('.code-editor')
  codeEditors.forEach((editor) => {
    const wrapper = editor.closest('.code-block-wrapper') as HTMLElement | null
    const lang = wrapper?.getAttribute('data-language') || 'text'
    if (!editor.classList.contains('hljs')) {
      editor.classList.add('hljs')
    }
    if (document.activeElement !== editor) {
      const code = editor.innerText
      if (code.trim()) {
        try {
          const highlighted = hljs.highlight(code, { language: lang }).value
          editor.innerHTML = highlighted
        } catch (e) {
          console.error('Highlight error:', e)
        }
      } else {
        editor.innerHTML = ''
      }
    }
  })
}

onMounted(() => {
  if (!editorRef.value) return
  const nodes = Array.isArray(props.modelValue) && props.modelValue.length > 0 ? props.modelValue : defaultNodes()
  editorRef.value.innerHTML = nodesToHtml(nodes)
  nextTick(() => {
    updateFileIcons()
    updateCodeHighlighting()
    renderStaticIcons()
  })
})

watch(() => props.modelValue, (newVal) => {
  if (!isInternalUpdate.value && editorRef.value) {
    const nodes = Array.isArray(newVal) && newVal.length > 0 ? newVal : defaultNodes()
    editorRef.value.innerHTML = nodesToHtml(nodes)
    nextTick(() => {
      updateFileIcons()
      updateCodeHighlighting()
      renderStaticIcons()
    })
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

const execCommand = (command: string, value?: string) => {
  restoreSelection()
  if (command === 'formatBlock' && value) {
    document.execCommand('formatBlock', false, value)
  } else if (command === 'justifyLeft' || command === 'justifyCenter' || command === 'justifyRight') {
    document.execCommand(command, false)
  } else if (command === 'undo') {
    document.execCommand('undo', false)
  } else if (command === 'redo') {
    document.execCommand('redo', false)
  } else {
    document.execCommand(command, false, value)
  }
  handleInput()
}

function findNodeById(nodes: EditorNode[], id: string, type: 'image' | 'file'): EditorNode | null {
  for (const n of nodes) {
    if (n.type === type && (n.id === id || (n as { id?: string }).id === id)) {
      return n
    }
    if ('children' in n && Array.isArray(n.children)) {
      const found = findNodeById(n.children, id, type)
      if (found) return found
    }
  }
  return null
}

const selectedImageNode = computed(() => {
  if (!selectedImageId.value || !Array.isArray(props.modelValue)) return null
  return findNodeById(props.modelValue, selectedImageId.value, 'image') as (EditorNode & { type: 'image' }) | null
})

const selectedFileNode = computed(() => {
  if (!selectedFileId.value || !Array.isArray(props.modelValue)) return null
  return findNodeById(props.modelValue, selectedFileId.value, 'file') as (EditorNode & { type: 'file' }) | null
})

function findTaskItemById(nodes: EditorNode[], id: string): (EditorNode & { type: 'taskItem'; checked: boolean; children: EditorNode[] }) | null {
  for (const n of nodes) {
    if (n.type === 'taskItem' && (n.id === id || (n as { id?: string }).id === id))
      return n as EditorNode & { type: 'taskItem'; checked: boolean; children: EditorNode[] }
    if ('children' in n && Array.isArray(n.children)) {
      const found = findTaskItemById(n.children, id)
      if (found) return found
    }
  }
  return null
}

function updateTaskItemCheckedInTree(nodes: EditorNode[], taskItemId: string, checked: boolean): EditorNode[] {
  return nodes.map((n) => {
    if (n.type === 'taskItem' && ((n as { id?: string }).id === taskItemId)) {
      return { ...n, checked } as EditorNode
    }
    if ('children' in n && Array.isArray(n.children)) {
      return { ...n, children: updateTaskItemCheckedInTree(n.children, taskItemId, checked) } as EditorNode
    }
    return n
  })
}

function onEditorClick(e: MouseEvent) {
  const target = e.target as HTMLElement
  const foldCopy = (target.classList.contains('fold-copy-btn') ? target : target.closest('.fold-copy-btn')) as HTMLElement | null
  if (foldCopy) {
    e.preventDefault()
    const block = foldCopy.closest('.code-block-wrapper') as HTMLElement | null
    const editor = block?.querySelector('.code-editor') as HTMLElement | null
    if (editor) {
      const code = editor.innerText
      navigator.clipboard.writeText(code).then(() => {
        ElMessage.success(t('editor.copySuccess'))
      })
    }
    return
  }
  const foldInsert = (target.classList.contains('fold-insert-btn') ? target : target.closest('.fold-insert-btn')) as HTMLElement | null
  if (foldInsert) {
    e.preventDefault()
    const block = foldInsert.closest('.fold-block, .code-block-wrapper') as HTMLElement | null
    if (!block) return
    const wrapperParent = block.parentElement
    if (!wrapperParent) return
    const newP = document.createElement('p')
    newP.className = 'editor-block'
    newP.setAttribute('data-id', genId())
    const br = document.createElement('br')
    newP.appendChild(br)
    if (block.nextSibling) {
      wrapperParent.insertBefore(newP, block.nextSibling)
    } else {
      wrapperParent.appendChild(newP)
    }
    const sel = window.getSelection()
    if (sel) {
      const newRange = document.createRange()
      newRange.setStart(newP, 0)
      newRange.collapse(true)
      sel.removeAllRanges()
      sel.addRange(newRange)
    }
    handleInput()
    return
  }
  const foldToggle = target.classList.contains('fold-toggle-btn') ? target : target.closest('.fold-toggle-btn')
  if (foldToggle) {
    e.preventDefault()
    const foldBlock = foldToggle.closest('.fold-block') as HTMLElement | null
    if (!foldBlock) return
    const id = foldBlock.getAttribute('data-id')
    if (!id || !Array.isArray(props.modelValue)) return
    
    function toggleFoldInTree(nodes: EditorNode[], targetId: string): EditorNode[] {
      return nodes.map(n => {
        if (n.type === 'fold' && n.id === targetId) {
          return { ...n, folded: !n.folded }
        }
        if ('children' in n && Array.isArray(n.children)) {
          return { ...n, children: toggleFoldInTree(n.children, targetId) }
        }
        return n
      })
    }
    
    const updated = toggleFoldInTree(props.modelValue, id)
    emit('update:modelValue', updated)
    isInternalUpdate.value = true
    if (editorRef.value) editorRef.value.innerHTML = nodesToHtml(updated)
    nextTick(() => {
      updateFileIcons()
      updateCodeHighlighting()
    })
    setTimeout(() => { isInternalUpdate.value = false }, 0)
    return
  }
  const codeCard = target.closest('.code-card') as HTMLElement | null
  if (codeCard) {
    const editor = codeCard.querySelector('.code-editor') as HTMLElement | null
    if (editor && target !== editor) {
      editor.focus()
    }
    selectedImageId.value = null
    selectedFileId.value = null
    return
  }
  const foldBlockStructure = target.closest('.fold-block') as HTMLElement | null
  if (foldBlockStructure && !target.closest('.fold-content')) {
    const content = foldBlockStructure.querySelector('.fold-content') as HTMLElement | null
    if (content) {
      content.focus()
    }
    return
  }
  const taskCheckbox = target.classList.contains('task-item-checkbox') ? target : target.closest('.task-item-checkbox')
  if (taskCheckbox) {
    e.preventDefault()
    const li = (taskCheckbox as HTMLElement).closest('li.task-item') as HTMLElement | null
    if (!li || !Array.isArray(props.modelValue)) return
    const taskItemId = li.getAttribute('data-id') ?? null
    if (!taskItemId) return
    const current = findTaskItemById(props.modelValue, taskItemId)
    if (!current) return
    const nextChecked = !current.checked
    const updated = updateTaskItemCheckedInTree(props.modelValue, taskItemId, nextChecked)
    emit('update:modelValue', updated)
    isInternalUpdate.value = true
    if (editorRef.value) editorRef.value.innerHTML = nodesToHtml(updated)
    nextTick(() => {
      updateFileIcons()
      updateCodeHighlighting()
    })
    setTimeout(() => { isInternalUpdate.value = false }, 0)
    return
  }
  if (target.closest('.image-toolbar-root') || target.closest('.file-toolbar-root')) return
  
  // 处理图片点击（包括折叠块内的图片）
  const imgElement = target.tagName === 'IMG' && target.classList.contains('image-block') 
    ? target as HTMLElement 
    : target.closest('img.image-block') as HTMLElement | null
  const imageWrapper = target.closest('.image-block-wrapper') as HTMLElement | null
  
  if (imgElement || imageWrapper) {
    const img = imgElement ?? imageWrapper?.querySelector('img.image-block') as HTMLElement | null
    if (img) {
      const id = img.getAttribute('data-id') ?? null
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
    }
    selectedFileId.value = null
    return
  }
  
  // 处理文件点击（包括折叠块内的文件）
  const fileWrapper = target.closest('.file-block-wrapper') as HTMLElement | null
  const fileCard = target.closest('.file-card') as HTMLElement | null
  const fileBlock = target.closest('.file-block') as HTMLElement | null
  
  if (fileWrapper || fileCard || fileBlock) {
    const wrapper = fileWrapper ?? (fileCard || fileBlock)?.closest('.file-block-wrapper') as HTMLElement | null
    if (wrapper) {
      const id = wrapper.getAttribute('data-id') ?? null
      if (id) {
        selectedFileId.value = id
        const block = wrapper.querySelector('.file-block') as HTMLElement | null
        const rect = (block ?? wrapper)?.getBoundingClientRect()
        if (rect) {
          let top: number
          if (rect.top - FILE_TOOLBAR_APPROX_HEIGHT - IMAGE_TOOLBAR_PADDING < 0) {
            top = rect.bottom + window.scrollY + IMAGE_TOOLBAR_PADDING
          } else {
            top = rect.top + window.scrollY - FILE_TOOLBAR_APPROX_HEIGHT - IMAGE_TOOLBAR_PADDING
          }
          let left = rect.left + rect.width / 2 - FILE_TOOLBAR_APPROX_WIDTH / 2
          left += window.scrollX
          left = Math.max(IMAGE_TOOLBAR_PADDING, Math.min(window.innerWidth - FILE_TOOLBAR_APPROX_WIDTH - IMAGE_TOOLBAR_PADDING + window.scrollX, left))
          selectedFileRect.value = { top, left }
        }
      }
    }
    selectedImageId.value = null
    return
  }
  selectedImageId.value = null
  selectedFileId.value = null
}

function onEditorContextMenu(e: MouseEvent) {
  const target = e.target as HTMLElement
  if (target.closest('.image-toolbar-root')) return
  
  // 处理图片右键（包括折叠块内的图片）
  const imgElement = target.tagName === 'IMG' && target.classList.contains('image-block') 
    ? target as HTMLElement 
    : target.closest('img.image-block') as HTMLElement | null
  const imageWrapper = target.closest('.image-block-wrapper') as HTMLElement | null
  
  if (imgElement || imageWrapper) {
    const img = imgElement ?? imageWrapper?.querySelector('img.image-block') as HTMLElement | null
    const wrapper = imageWrapper ?? (imgElement?.closest('.image-block-wrapper') as HTMLElement | null)
    if (img || wrapper) {
      const el = img ?? wrapper!
      const assetPath = el.getAttribute('data-asset-path') ?? ''
      const id = el.getAttribute('data-id') ?? ''
      if (assetPath) {
        e.preventDefault()
        emit('contextmenu', { type: 'image', assetPath, id, clientX: e.clientX, clientY: e.clientY })
      }
    }
    return
  }
  
  // 处理文件右键（包括折叠块内的文件）
  const fileWrapper = target.closest('.file-block-wrapper') as HTMLElement | null
  const fileCard = target.closest('.file-card') as HTMLElement | null
  const fileBlock = target.closest('.file-block') as HTMLElement | null
  
  if (fileWrapper || fileCard || fileBlock) {
    const wrapper = fileWrapper ?? (fileCard || fileBlock)?.closest('.file-block-wrapper') as HTMLElement | null
    if (wrapper) {
      const assetPath = wrapper.getAttribute('data-asset-path') ?? ''
      const id = wrapper.getAttribute('data-id') ?? ''
      if (assetPath) {
        e.preventDefault()
        emit('contextmenu', { type: 'file', assetPath, id, clientX: e.clientX, clientY: e.clientY })
      }
    }
  }
}

function updateNodeInTree(nodes: EditorNode[], id: string, type: 'image' | 'file', updates: { widthPercent?: number; align?: 'left' | 'center' | 'right' }): EditorNode[] {
  return nodes.map(node => {
    if (node.type === type && (node.id === id || (node as { id?: string }).id === id)) {
      return { ...node, ...updates } as EditorNode
    }
    if ('children' in node && Array.isArray(node.children)) {
      return { ...node, children: updateNodeInTree(node.children, id, type, updates) } as EditorNode
    }
    return node
  })
}

function updateImageNode(updates: { widthPercent?: number; align?: 'left' | 'center' | 'right' }) {
  if (!selectedImageId.value || !Array.isArray(props.modelValue)) return
  const next = updateNodeInTree(props.modelValue, selectedImageId.value, 'image', updates)
  emit('update:modelValue', next)
  isInternalUpdate.value = true
  if (editorRef.value) editorRef.value.innerHTML = nodesToHtml(next)
  nextTick(() => {
    updateFileIcons()
    updateCodeHighlighting()
  })
  setTimeout(() => { isInternalUpdate.value = false }, 0)
}

function updateFileNode(updates: { align?: 'left' | 'center' | 'right' }) {
  if (!selectedFileId.value || !Array.isArray(props.modelValue)) return
  const next = updateNodeInTree(props.modelValue, selectedFileId.value, 'file', updates)
  emit('update:modelValue', next)
  isInternalUpdate.value = true
  if (editorRef.value) editorRef.value.innerHTML = nodesToHtml(next)
  nextTick(() => {
    updateFileIcons()
    updateCodeHighlighting()
  })
  setTimeout(() => { isInternalUpdate.value = false }, 0)
}

function onEditorDblClick(e: MouseEvent) {
  const target = e.target as HTMLElement
  const imgElement = target.tagName === 'IMG' && target.classList.contains('image-block') 
    ? target as HTMLElement 
    : target.closest('img.image-block') as HTMLElement | null
  const imageWrapper = target.closest('.image-block-wrapper') as HTMLElement | null
  
  if (imgElement || imageWrapper) {
    const img = imgElement ?? imageWrapper?.querySelector('img.image-block') as HTMLImageElement | null
    if (img) {
      const assetPath = img.getAttribute('data-asset-path') ?? ''
      const id = img.getAttribute('data-id') ?? ''
      if (assetPath && id) {
        emit('open-asset', { type: 'image', assetPath, id })
      }
    }
  }
}

function computeCursorBlockIndex(): number {
  const sel = window.getSelection()
  const editor = editorRef.value
  if (!sel || sel.rangeCount === 0 || !editor) return -1
  
  const anchorNode = sel.anchorNode
  if (!anchorNode) return -1

  // 寻找所在的 block-container
  let container: HTMLElement | null = null
  let curr: Node | null = anchorNode
  while (curr && curr !== editor) {
    if (curr instanceof HTMLElement && curr.getAttribute('contenteditable') === 'true') {
      container = curr
      break
    }
    curr = curr.parentNode
  }
  if (!container) container = editor

  let node: Node | null = anchorNode
  while (node && node.parentNode !== container) node = node.parentNode
  if (!node) return -1
  
  const children = container.childNodes
  for (let i = 0; i < children.length; i++) {
    if (children[i] === node) return i
  }
  return -1
}

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

function insertTaskListAtSelection() {
  restoreSelection()
  const info = getInsertionContainer()
  if (!info.container) return
  const taskNode: EditorNode = {
    type: 'taskList',
    id: genId(),
    children: [{ type: 'taskItem', id: genId(), checked: false, children: [] }]
  }
  const html = nodeToHtml(taskNode)
  const wrap = document.createElement('div')
  wrap.innerHTML = html
  const ul = wrap.firstElementChild
  if (!ul) return
  
  if (info.index >= 0 && info.index < info.container.childNodes.length) {
    const next = info.container.childNodes[info.index + 1] || null
    info.container.insertBefore(ul, next)
  } else {
    info.container.appendChild(ul)
  }
  handleInput()
}

function getFoldDepth(node: Node | null): number {
  let depth = 0
  let curr = node
  while (curr && curr !== editorRef.value) {
    if (curr instanceof HTMLElement && curr.classList.contains('fold-block')) {
      depth++
    }
    curr = curr.parentNode
  }
  return depth
}

function insertCodeBlock() {
  restoreSelection()
  const info = getInsertionContainer()
  if (!info.container) return
  const codeNode: EditorNode = {
    type: 'code',
    id: genId(),
    content: '',
    language: 'javascript'
  }
  const html = nodeToHtml(codeNode)
  const wrap = document.createElement('div')
  wrap.innerHTML = html
  const el = wrap.firstElementChild
  if (!el) return
  
  if (info.index >= 0 && info.index < info.container.childNodes.length) {
    const next = info.container.childNodes[info.index + 1] || null
    info.container.insertBefore(el, next)
  } else {
    info.container.appendChild(el)
  }
  handleInput()
}

function insertFoldBlock() {
  restoreSelection()
  const sel = window.getSelection()
  if (!sel || sel.rangeCount === 0 || !editorRef.value) return
  
  const depth = getFoldDepth(sel.anchorNode)
  if (depth >= 3) {
    return 
  }

  const info = getInsertionContainer()
  if (!info.container) return
  const foldNode: EditorNode = {
    type: 'fold',
    id: genId(),
    folded: false,
    children: [{ type: 'p', id: genId(), children: [] }]
  }
  const html = nodeToHtml(foldNode)
  const wrap = document.createElement('div')
  wrap.innerHTML = html
  const el = wrap.firstElementChild
  if (!el) return
  
  if (info.index >= 0 && info.index < info.container.childNodes.length) {
    const next = info.container.childNodes[info.index + 1] || null
    info.container.insertBefore(el, next)
  } else {
    info.container.appendChild(el)
  }
  handleInput()
}

function insertNodesAtSelection(newNodes: EditorNode | EditorNode[]) {
  restoreSelection()
  const info = getInsertionContainer()
  if (!info.container) return

  const nodesArray = Array.isArray(newNodes) ? newNodes : [newNodes]
  const frag = document.createDocumentFragment()
  for (const node of nodesArray) {
    const html = nodeToHtml(node)
    const wrap = document.createElement('div')
    wrap.innerHTML = html
    const el = wrap.firstElementChild
    if (el) frag.appendChild(el)
  }

  if (!frag.childNodes.length) return

  if (info.index >= 0 && info.index < info.container.childNodes.length) {
    const next = info.container.childNodes[info.index + 1] || null
    info.container.insertBefore(frag, next)
  } else {
    info.container.appendChild(frag)
  }
  handleInput()
}

defineExpose({
  execCommand,
  handleInput,
  saveSelection,
  restoreSelection,
  getCursorBlockIndex,
  insertTaskListAtSelection,
  insertPlainText,
  insertCodeBlock,
  insertFoldBlock,
  insertNodesAtSelection
})
</script>

<template>
  <div class="advanced-editor-container">
    <div
      ref="editorRef"
      class="advanced-editor"
      contenteditable="true"
      @input="handleInput"
      @paste="handlePaste"
      @keydown.enter.capture="handleEnterKey"
      @beforeinput="handleBeforeInput"
      @click="onEditorClick"
      @dblclick="onEditorDblClick"
      @contextmenu="onEditorContextMenu"
      @keydown="handleKeyDown"
      @focusout="updateCodeHighlighting"
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
      <div
        v-if="selectedFileId && selectedFileNode"
        ref="fileToolbarRef"
        class="file-toolbar-root file-toolbar"
        :style="{ top: selectedFileRect.top + 'px', left: selectedFileRect.left + 'px' }"
      >
        <button type="button" class="file-toolbar-btn" title="左对齐" @click="updateFileNode({ align: 'left' })">
          <AlignLeft :size="16" />
        </button>
        <button type="button" class="file-toolbar-btn" title="居中" @click="updateFileNode({ align: 'center' })">
          <AlignCenter :size="16" />
        </button>
        <button type="button" class="file-toolbar-btn" title="右对齐" @click="updateFileNode({ align: 'right' })">
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

:deep(.task-list) {
  list-style: none;
  padding-left: 0;
  margin: 0.5em 0;
}

:deep(.task-item) {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  margin: 0.35em 0;
}

:deep(.task-item-checkbox) {
  flex-shrink: 0;
  margin-top: 0.25em;
  cursor: pointer;
}

:deep(.task-item-content) {
  flex: 1;
  min-width: 0;
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
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  flex-shrink: 0;
  position: relative;
}

:deep(.file-icon-fallback) {
  font-size: 20px;
  line-height: 1;
}

:deep(.file-icon-img) {
  width: 20px;
  height: 20px;
  object-fit: contain;
  display: none;
}

:deep(.file-icon-img[src]) {
  display: block;
}

:deep(.file-icon:has(.file-icon-img[src]) .file-icon-fallback) {
  display: none;
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

/* Code Block Styles */
:deep(.code-block-wrapper) {
  position: relative;
}

:deep(.code-card) {
  display: flex;
  flex-direction: column;
  border: 1px solid var(--file-card-border);
  border-radius: 8px;
  background: var(--file-card-bg);
  overflow: hidden;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.05);
}

:deep(.code-header) {
  padding: 8px 16px;
  background: rgba(0, 0, 0, 0.04);
  border-bottom: 1px solid var(--file-card-border);
  font-size: 11px;
  font-weight: 600;
  color: var(--file-size-color);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

:deep(.code-header-btns) {
  display: flex;
  gap: 6px;
  align-items: center;
}

.dark :deep(.code-header) {
  background: rgba(255, 255, 255, 0.04);
}

:deep(.code-editor) {
  padding: 16px;
  font-family: 'Fira Code', 'Consolas', 'Monaco', 'Courier New', monospace;
  font-size: 13px;
  line-height: 1.6;
  white-space: pre;
  overflow-x: auto;
  word-break: normal;
  outline: none;
  color: var(--app-text-color);
  min-height: 1em;
}

/* Fold Block Styles */
:deep(.fold-block) {
  margin: 20px 0;
  border: none;
  padding: 0;
}

:deep(.fold-line-top), :deep(.fold-line-bottom) {
  height: 1px;
  background: var(--file-card-border);
  margin: 4px 0;
  width: 100%;
  opacity: 0.8;
}

:deep(.fold-header) {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 4px 0;
  color: var(--file-size-color);
  font-size: 13px;
  font-weight: 500;
  user-select: none;
}

:deep(.fold-toggle-btn), :deep(.fold-insert-btn), :deep(.fold-copy-btn) {
  background: rgba(0, 0, 0, 0.05);
  border: none;
  cursor: pointer;
  padding: 0;
  color: var(--app-text-color);
  font-size: 10px;
  width: 22px;
  height: 22px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 6px;
  transition: background 0.2s;
}

:deep(.fold-toggle-btn:hover), :deep(.fold-insert-btn:hover), :deep(.fold-copy-btn:hover) {
  background: rgba(0, 0, 0, 0.1);
}

.dark :deep(.fold-toggle-btn), .dark :deep(.fold-insert-btn), .dark :deep(.fold-copy-btn) {
  background: rgba(255, 255, 255, 0.1);
}

.dark :deep(.fold-toggle-btn:hover), .dark :deep(.fold-insert-btn:hover), .dark :deep(.fold-copy-btn:hover) {
  background: rgba(255, 255, 255, 0.15);
}

:deep(.fold-insert-btn) {
  font-size: 14px;
  margin-left: 4px;
}

:deep(.fold-copy-btn) {
  display: flex;
  align-items: center;
  justify-content: center;
}

:deep(.fold-copy-btn .copy-icon-placeholder) {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
}

:deep(.fold-copy-btn .copy-icon-placeholder svg) {
  width: 14px;
  height: 14px;
  stroke: currentColor;
}

:deep(.fold-content) {
  padding: 4px 0 4px 12px;
  border-left: 1px solid var(--file-card-border);
  margin-left: 10px;
  outline: none;
}

/* Nesting support */
:deep(.fold-content .fold-block) {
  margin: 8px 0;
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

.file-toolbar {
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

.dark .file-toolbar {
  background: var(--app-surface-color);
  border-color: rgba(255, 255, 255, 0.15);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
}

.file-toolbar-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  padding: 0;
  border: none;
  background: transparent;
  border-radius: 4px;
  cursor: pointer;
  color: var(--app-text-color);
  transition: background 0.2s;
}

.file-toolbar-btn:hover {
  background: rgba(0, 0, 0, 0.08);
}

.dark .file-toolbar-btn:hover {
  background: rgba(255, 255, 255, 0.15);
}
</style>
