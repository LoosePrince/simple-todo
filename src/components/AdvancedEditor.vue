<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { ElMessage } from 'element-plus';
import hljs from 'highlight.js';
import 'highlight.js/styles/github.css';
import { AlignCenter, AlignLeft, AlignRight, ChevronDown, ChevronUp, Copy, Minus, Plus, Trash2 } from 'lucide-vue-next';
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
const { t } = useI18n()

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

const foldTitleText = computed(() => t('editor.foldTitle'))
const foldInsertAfterText = computed(() => t('editor.foldInsertAfter'))

/** 代码块可选语言（与 highlight.js 别名一致） */
const CODE_LANGUAGES = [
  { label: 'Text', value: 'text' },
  { label: 'JavaScript', value: 'javascript' },
  { label: 'TypeScript', value: 'typescript' },
  { label: 'Python', value: 'python' },
  { label: 'JSON', value: 'json' },
  { label: 'HTML', value: 'html' },
  { label: 'CSS', value: 'css' },
  { label: 'XML', value: 'xml' },
  { label: 'Markdown', value: 'markdown' },
  { label: 'Bash', value: 'bash' },
  { label: 'SQL', value: 'sql' }
]

// --- Recursive HTML Renderer ---

const nodeToHtml = (node: EditorNode): string => {
  if (node.type === 'text') return escapeHtml(node.value)

  if (node.type === 'image') {
    const w = node.widthPercent ?? 100
    const align = node.align ?? 'left'
    const ap = node.assetPath ?? ''
    return `<span class="image-block-wrapper editor-block" data-id="${node.id}" data-align="${align}" data-width-percent="${w}" data-asset-path="${escapeHtml(ap)}" style="display: block; text-align: ${align}; margin: 8px 0;"><img class="image-block" data-id="${node.id}" data-width-percent="${w}" data-align="${align}" data-asset-path="${escapeHtml(ap)}" src="${node.url}" style="width: ${w}%; max-width: 100%; display: inline-block; vertical-align: middle; border-radius: 8px; cursor: pointer;" contenteditable="false" /><br class="asset-trailing-br"></span>`
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
    return `<span class="editor-block code-block-wrapper" data-id="${node.id}" data-language="${escapeHtml(lang)}" style="display: block; margin: 12px 0;"><span contenteditable="false" style="display: inline-block; vertical-align: middle; width: 100%;"><span class="code-card"><div class="code-editor" contenteditable="true" spellcheck="false">${escapeHtml(node.content)}</div></span></span><br class="asset-trailing-br"></span>`
  }

  if (node.type === 'fold') {
    const folded = node.folded ?? false
    const title = escapeHtml(foldTitleText.value)
    const childrenHtml = (node.children || []).map(nodeToHtml).join('') || '<p class="editor-block"><br></p>'
    return `<span class="editor-block fold-block" data-id="${node.id}" data-folded="${folded}" style="display: block; margin: 20px 0;"><span contenteditable="false" style="display: inline-block; vertical-align: middle; width: 100%;"><div class="fold-line-top"></div><div class="fold-header"><span class="fold-title">${title}</span></div><div class="fold-content" contenteditable="true" style="display: ${folded ? 'none' : 'block'};">${childrenHtml}</div><div class="fold-line-bottom"></div></span><br class="asset-trailing-br"></span>`
  }

  if (node.type === 'strong') return `<strong>${(node.children || []).map(nodeToHtml).join('')}</strong>`
  if (node.type === 'em') return `<em>${(node.children || []).map(nodeToHtml).join('')}</em>`
  if (node.type === 'color') return `<span style="color: ${escapeHtml(node.color)}">${(node.children || []).map(nodeToHtml).join('')}</span>`

  if (node.type === 'li') {
    const inner = (node.children || []).map(nodeToHtml).join('') || '<br>'
    return `<li class="editor-block" data-id="${node.id ?? genId()}">${inner}</li>`
  }

  if (node.type === 'ul' || node.type === 'ol') {
    const inner = (node.children || []).map(nodeToHtml).join('') || '<li><br></li>'
    return `<${node.type} class="editor-block list-block" data-id="${node.id ?? genId()}">${inner}</${node.type}>`
  }

  if (node.type === 'taskList') {
    const inner = (node.children || []).map(nodeToHtml).join('')
    return `<ul class="editor-block task-list" data-id="${node.id ?? genId()}" data-type="task">${inner || '<li class="editor-block task-item" data-id="' + genId() + '" data-checked="false"><input type="checkbox" class="task-item-checkbox" contenteditable="false"><span class="task-item-content"><br></span></li>'}</ul>`
  }

  if (node.type === 'taskItem') {
    const checked = node.checked ?? false
    const inner = (node.children || []).map(nodeToHtml).join('') || '<br>'
    return `<li class="editor-block task-item" data-id="${node.id ?? genId()}" data-checked="${checked}"><input type="checkbox" class="task-item-checkbox" contenteditable="false"${checked ? ' checked' : ''}><span class="task-item-content">${inner}</span></li>`
  }

  if (node.type === 'p' || node.type === 'h1' || node.type === 'h2') {
    const inner = (node.children || []).map(nodeToHtml).join('') || '<br>'
    const align = (node as any).align
    const alignStyle = align && align !== 'left' ? ` style="text-align: ${align}"` : ''
    const alignData = align ? ` data-align="${align}"` : ''
    return `<${node.type} class="editor-block" data-id="${node.id ?? genId()}"${alignData}${alignStyle}>${inner}</${node.type}>`
  }

  return ''
}

const nodesToHtml = (nodes: EditorNode[]) => nodes.map(nodeToHtml).join('')

// --- Decoration Logic ---

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
    } catch (_) { }
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

function postRender() {
  updateFileIcons()
  updateCodeHighlighting()
  renderStaticIcons()
}

// --- DOM to JSON Parser ---

function getBlockAlign(el: HTMLElement): 'left' | 'center' | 'right' {
  const raw = el.style?.textAlign || el.getAttribute('data-align') || (typeof getComputedStyle !== 'undefined' ? getComputedStyle(el).textAlign : '')
  if (raw === 'center') return 'center'
  if (raw === 'right') return 'right'
  return 'left'
}

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
        const al = child.getAttribute('data-align') ?? img.getAttribute('data-align')
        const ap = child.getAttribute('data-asset-path') ?? img.getAttribute('data-asset-path') ?? child.getAttribute('data-asset-path') ?? ''
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
        const al = child.getAttribute('data-align') ?? img.getAttribute('data-align')
        const ap = child.getAttribute('data-asset-path') ?? img.getAttribute('data-asset-path') ?? child.getAttribute('data-asset-path') ?? ''
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

// --- Main Event Handlers ---

const handleInput = () => {
  isInternalUpdate.value = true
  emit('update:modelValue', domToNodes())
  setTimeout(() => isInternalUpdate.value = false, 0)
}

function insertPlainText(text: string) {
  if (!text) return
  const normalized = text.replace(/\r\n/g, '\n')
  document.execCommand('insertText', false, normalized)
  handleInput()
}

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
  if (!node) return { container, index: -1 }
  const kids = container.childNodes
  for (let i = 0; i < kids.length; i++) {
    if (kids[i] === node) return { container, index: i }
  }
  return { container, index: -1 }
}

function handleBeforeInput(e: Event) {
  const inputEvent = e as InputEvent
  const selection = window.getSelection()
  if (!selection || selection.rangeCount === 0) return
  const range = selection.getRangeAt(0)
  const container = range.commonAncestorContainer
  const containerEl = container.nodeType === Node.ELEMENT_NODE ? container as HTMLElement : container.parentElement as HTMLElement | null
  const foldContent = containerEl?.closest('.fold-content') as HTMLElement | null
  const codeEditor = containerEl?.closest('.code-editor') as HTMLElement | null

  if (codeEditor && !foldContent) return
  if (foldContent && !codeEditor) {
    const isInAsset = containerEl?.closest('.image-block-wrapper, .file-block-wrapper, .code-block-wrapper')
    const isInFoldHeader = containerEl?.closest('.fold-header')
    if (!isInAsset && !isInFoldHeader) return
  }

  // 特殊处理退格键
  if (inputEvent.inputType === 'deleteContentBackward' && range.collapsed) {
    const wrapper = containerEl?.closest('.editor-block') as HTMLElement | null
    if (wrapper) {
      const blockStartRange = document.createRange()
      blockStartRange.selectNodeContents(wrapper)
      blockStartRange.collapse(true)
      const atBlockStart = range.compareBoundaryPoints(Range.START_TO_START, blockStartRange) <= 0

      if (atBlockStart) {
        const prev = wrapper.previousElementSibling as HTMLElement | null
        if (prev) {
          e.preventDefault()
          const isSpecial = prev.classList.contains('image-block-wrapper') ||
            prev.classList.contains('file-block-wrapper') ||
            prev.classList.contains('code-block-wrapper') ||
            prev.classList.contains('fold-block')

          if (isSpecial) {
            // 如果前一个是特殊块，先选中它
            const newRange = document.createRange()
            newRange.selectNode(prev)
            selection.removeAllRanges()
            selection.addRange(newRange)
          } else {
            // 如果是普通段落且为空，直接删除
            if ((prev.innerText || '').trim() === '') {
              prev.remove()
            } else {
              // 否则将当前内容合并到上一行末尾
              const endRange = document.createRange()
              endRange.selectNodeContents(prev)
              endRange.collapse(false)
              selection.removeAllRanges()
              selection.addRange(endRange)
              document.execCommand('deleteContentBackward')
            }
          }
          handleInput()
          return
        }
      }
    }
  }

  if (codeEditor && foldContent) {
    const codeBlockWrapper = codeEditor.closest('.code-block-wrapper') as HTMLElement | null
    if (codeBlockWrapper && inputEvent.inputType === 'deleteContentBackward' && range.collapsed) {
      const blockStartRange = document.createRange()
      blockStartRange.selectNodeContents(codeBlockWrapper)
      blockStartRange.collapse(true)
      if (range.compareBoundaryPoints(Range.START_TO_START, blockStartRange) <= 0) {
        const prev = codeBlockWrapper.previousElementSibling as HTMLElement | null
        if (prev) {
          e.preventDefault()
          if ((prev.innerText || '').trim() === '') {
            prev.remove()
            const startRange = document.createRange()
            startRange.selectNodeContents(codeBlockWrapper)
            startRange.collapse(true)
            selection.removeAllRanges()
            selection.addRange(startRange)
          } else {
            const endRange = document.createRange()
            endRange.selectNodeContents(prev)
            endRange.collapse(false)
            selection.removeAllRanges()
            selection.addRange(endRange)
            document.execCommand('deleteContentBackward')
          }
          handleInput()
          return
        }
      }
    }
    return
  }

  const wrapper = containerEl?.closest('.image-block-wrapper, .file-block-wrapper, .file-block, .code-block-wrapper, .fold-block') as HTMLElement | null
  if (!wrapper) return

  const img = wrapper.querySelector('img.image-block')
  const fileCard = wrapper.querySelector('.file-card')
  const codeCard = wrapper.querySelector('.code-card')
  const isFold = wrapper.classList.contains('fold-block')
  const trailingBr = wrapper.querySelector('.asset-trailing-br')
  const isInAssetStructure = img?.contains(container) || fileCard?.contains(container) || (codeCard?.contains(container) && !containerEl?.closest('.code-editor')) || (isFold && containerEl?.closest('.fold-header'))

  if (isInAssetStructure && inputEvent.inputType !== 'deleteContentBackward') {
    e.preventDefault()
    return
  }

  const isAtTrailingBrOrEnd = (trailingBr && (container === trailingBr || container.parentElement === trailingBr.parentElement)) || (container === wrapper && range.startOffset >= (wrapper.childNodes.length || 1))
  if (isAtTrailingBrOrEnd) {
    if (inputEvent.inputType === 'insertLineBreak' || (inputEvent.inputType === 'insertText' && inputEvent.data === '\n')) return
    e.preventDefault()
    const wrapperParent = wrapper.parentElement
    if (!wrapperParent) return
    const newP = document.createElement('p')
    newP.className = 'editor-block'
    newP.setAttribute('data-id', genId())
    newP.appendChild(document.createElement('br'))
    if (wrapper.nextSibling) wrapperParent.insertBefore(newP, wrapper.nextSibling)
    else wrapperParent.appendChild(newP)
    const newRange = document.createRange()
    newRange.setStart(newP, 0)
    newRange.collapse(true)
    selection.removeAllRanges()
    selection.addRange(newRange)
    if (inputEvent.inputType === 'insertText' && inputEvent.data) document.execCommand('insertText', false, inputEvent.data)
    handleInput()
    return
  }
  if (wrapper.classList.contains('fold-block') && foldContent && foldContent.contains(container)) {
    if (!containerEl?.closest('.image-block-wrapper, .file-block-wrapper, .code-block-wrapper')) return
  }
  if (wrapper.contains(container) && !isInAssetStructure && container !== trailingBr && container.parentElement !== trailingBr?.parentElement) {
    e.preventDefault()
    const nextSibling = wrapper.nextSibling
    if (nextSibling) {
      const newRange = document.createRange()
      if (nextSibling.nodeType === Node.ELEMENT_NODE) newRange.setStart(nextSibling as Node, 0)
      else newRange.setStartAfter(wrapper)
      newRange.collapse(true)
      selection.removeAllRanges()
      selection.addRange(newRange)
    }
  }
}


function handleEnterKey(e: KeyboardEvent) {
  const selection = window.getSelection()
  if (!selection || selection.rangeCount === 0 || !editorRef.value) return
  const range = selection.getRangeAt(0)
  const container = range.commonAncestorContainer
  const containerEl = container.nodeType === Node.ELEMENT_NODE ? container as HTMLElement : container.parentElement as HTMLElement | null
  if (containerEl?.closest('.code-editor')) return
  const wrapper = containerEl?.closest('.image-block-wrapper, .file-block-wrapper, .file-block, .code-block-wrapper, .fold-block') as HTMLElement | null
  if (!wrapper) return
  const img = wrapper.querySelector('img.image-block')
  const fileCard = wrapper.querySelector('.file-card')
  const codeCard = wrapper.querySelector('.code-card')
  const isFold = wrapper.classList.contains('fold-block')
  const trailingBr = wrapper.querySelector('.asset-trailing-br')
  const isInAssetStructure = img?.contains(container) || fileCard?.contains(container) || (codeCard?.contains(container) && !containerEl?.closest('.code-editor')) || (isFold && containerEl?.closest('.fold-header'))
  if (isInAssetStructure) return
  const isAtEndOfWrapper = container === wrapper && range.startOffset >= wrapper.childNodes.length
  const isAtTrailingBr = trailingBr && (container === trailingBr || container.parentElement === trailingBr.parentElement || range.startContainer === trailingBr)
  const isAfterAsset = (wrapper === container || wrapper.contains(container)) && (isAtTrailingBr || isAtEndOfWrapper || (container === wrapper && range.startOffset > 0))
  const wrapperParent = wrapper.parentElement
  const blockStartRange = document.createRange()
  blockStartRange.selectNodeContents(wrapper)
  blockStartRange.collapse(true)
  if ((wrapper === container || wrapper.contains(container)) && range.compareBoundaryPoints(Range.START_TO_START, blockStartRange) <= 0) {
    e.preventDefault()
    const newP = document.createElement('p')
    newP.className = 'editor-block'
    newP.setAttribute('data-id', genId())
    newP.appendChild(document.createElement('br'))
    if (wrapperParent) wrapperParent.insertBefore(newP, wrapper)
    const newR = document.createRange()
    newR.setStart(newP, 0)
    newR.collapse(true)
    selection.removeAllRanges()
    selection.addRange(newR)
    handleInput()
    return
  }
  if (isAfterAsset || (wrapperParent && container.parentElement && wrapperParent === container.parentElement && container.nodeType === Node.TEXT_NODE && Array.from(wrapperParent.childNodes).indexOf(wrapper) + 1 === Array.from(wrapperParent.childNodes).indexOf(container as ChildNode))) {
    e.preventDefault()
    const newP = document.createElement('p')
    newP.className = 'editor-block'
    newP.setAttribute('data-id', genId())
    newP.appendChild(document.createElement('br'))
    if (wrapperParent) {
      if (wrapper.nextSibling) wrapperParent.insertBefore(newP, wrapper.nextSibling)
      else wrapperParent.appendChild(newP)
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
      if (range.compareBoundaryPoints(Range.END_TO_END, lastRange) >= 0) {
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

// --- Internal Tree Helpers ---

function findNodeById(nodes: EditorNode[], id: string, type: 'image' | 'file' | 'code' | 'fold'): EditorNode | null {
  for (const n of nodes) {
    if (n.type === type && (n.id === id || (n as any).id === id)) return n
    if ('children' in n && Array.isArray(n.children)) {
      const found = findNodeById(n.children, id, type)
      if (found) return found
    }
  }
  return null
}

/** 从树中移除指定 id 的块（支持根级与折叠内嵌套） */
function removeBlockById(nodes: EditorNode[], id: string): EditorNode[] {
  return nodes.flatMap(n => {
    if ((n as any).id === id) return []
    if ('children' in n && Array.isArray(n.children))
      return [{ ...n, children: removeBlockById(n.children, id) } as EditorNode]
    return [n]
  })
}

function updateNodeInTree(nodes: EditorNode[], id: string, type: 'image' | 'file' | 'code' | 'fold', updates: any): EditorNode[] {
  return nodes.map(n => {
    if (n.type === type && (n.id === id || (n as any).id === id)) return { ...n, ...updates } as EditorNode
    if ('children' in n && Array.isArray(n.children)) return { ...n, children: updateNodeInTree(n.children, id, type, updates) } as EditorNode
    return n
  })
}

function updateTaskItemCheckedInTree(nodes: EditorNode[], taskItemId: string, checked: boolean): EditorNode[] {
  return nodes.map(n => {
    if (n.type === 'taskItem' && (n.id === taskItemId || (n as any).id === taskItemId)) return { ...n, checked } as EditorNode
    if ('children' in n && Array.isArray(n.children)) return { ...n, children: updateTaskItemCheckedInTree(n.children, taskItemId, checked) } as EditorNode
    return n
  })
}

// --- Lifecycle & Watch ---

const defaultNodes = (): EditorNode[] => [{ type: 'p', id: genId(), children: [] }]

onMounted(() => {
  if (editorRef.value) {
    const nodes = Array.isArray(props.modelValue) && props.modelValue.length > 0 ? props.modelValue : defaultNodes()
    editorRef.value.innerHTML = nodesToHtml(nodes)
    nextTick(() => postRender())
  }
  document.addEventListener('selectionchange', selectionChangeHandler)
})

onUnmounted(() => {
  document.removeEventListener('selectionchange', selectionChangeHandler)
})

watch(() => props.modelValue, (newVal) => {
  if (!isInternalUpdate.value && editorRef.value) {
    const nodes = Array.isArray(newVal) && newVal.length > 0 ? newVal : defaultNodes()
    editorRef.value.innerHTML = nodesToHtml(nodes)
    nextTick(() => postRender())
  }
}, { deep: true })

// --- State ---

const lastCursorBlockIndex = ref(-1)
const selectedImageId = ref<string | null>(null)
const selectedImageRect = ref({ top: 0, left: 0 })
const selectedFileId = ref<string | null>(null)
const selectedFileRect = ref({ top: 0, left: 0 })
const IMAGE_TOOLBAR_APPROX_HEIGHT = 44
const IMAGE_TOOLBAR_APPROX_WIDTH = 220
const FILE_TOOLBAR_APPROX_HEIGHT = 44
const FILE_TOOLBAR_APPROX_WIDTH = 180
const IMAGE_TOOLBAR_PADDING = 8

const selectedImageNode = computed(() => selectedImageId.value ? findNodeById(props.modelValue, selectedImageId.value, 'image') as any : null)
const selectedFileNode = computed(() => selectedFileId.value ? findNodeById(props.modelValue, selectedFileId.value, 'file') as any : null)

const selectedCodeBlockId = ref<string | null>(null)
const selectedCodeBlockRect = ref({ top: 0, left: 0 })
const selectedCodeBlockNode = computed(() => selectedCodeBlockId.value ? findNodeById(props.modelValue, selectedCodeBlockId.value, 'code') as any : null)

const selectedFoldBlockId = ref<string | null>(null)
const selectedFoldBlockRect = ref({ top: 0, left: 0 })
const selectedFoldBlockNode = computed(() => selectedFoldBlockId.value ? findNodeById(props.modelValue, selectedFoldBlockId.value, 'fold') as any : null)

const CODE_TOOLBAR_APPROX_HEIGHT = 44
const CODE_TOOLBAR_APPROX_WIDTH = 320
const FOLD_TOOLBAR_APPROX_HEIGHT = 44
const FOLD_TOOLBAR_APPROX_WIDTH = 180

function setCodeBlockLanguage(id: string, lang: string) {
  if (Array.isArray(props.modelValue)) {
    emit('update:modelValue', updateNodeInTree(props.modelValue, id, 'code', { language: lang }))
  }
}

function copyCodeBlockContent(id: string) {
  const el = editorRef.value
  if (!el) return
  const wrapper = el.querySelector(`.code-block-wrapper[data-id="${id}"]`) as HTMLElement | null
  const editor = wrapper?.querySelector('.code-editor') as HTMLElement | null
  if (editor) {
    navigator.clipboard.writeText(editor.innerText).then(() => ElMessage.success(t('editor.copySuccess')))
  }
}

function insertAfterBlock(blockEl: HTMLElement) {
  if (!blockEl.parentElement) return
  const newP = document.createElement('p')
  newP.className = 'editor-block'
  newP.setAttribute('data-id', genId())
  newP.appendChild(document.createElement('br'))
  if (blockEl.nextSibling) blockEl.parentElement.insertBefore(newP, blockEl.nextSibling)
  else blockEl.parentElement.appendChild(newP)
  const sel = window.getSelection()
  if (sel) {
    const newRange = document.createRange()
    newRange.setStart(newP, 0)
    newRange.collapse(true)
    sel.removeAllRanges()
    sel.addRange(newRange)
  }
  handleInput()
}

function deleteBlockById(id: string) {
  if (Array.isArray(props.modelValue)) {
    emit('update:modelValue', removeBlockById(props.modelValue, id))
  }
  selectedCodeBlockId.value = null
  selectedFoldBlockId.value = null
}

function toggleFoldBlock(id: string) {
  if (!selectedFoldBlockNode.value || !Array.isArray(props.modelValue)) return
  const folded = !selectedFoldBlockNode.value.folded
  emit('update:modelValue', updateNodeInTree(props.modelValue, id, 'fold', { folded }))
}

function insertAfterSelectedCodeBlock() {
  const id = selectedCodeBlockId.value
  const el = editorRef.value
  if (!id || !el) return
  const wrapper = el.querySelector(`.code-block-wrapper[data-id="${id}"]`) as HTMLElement | null
  if (wrapper) insertAfterBlock(wrapper)
}

function insertAfterSelectedFoldBlock() {
  const id = selectedFoldBlockId.value
  const el = editorRef.value
  if (!id || !el) return
  const wrapper = el.querySelector(`.fold-block[data-id="${id}"]`) as HTMLElement | null
  if (wrapper) insertAfterBlock(wrapper)
}

function removeEmptyLinesAroundSelectedCodeBlock() {
  const id = selectedCodeBlockId.value
  const el = editorRef.value
  if (!id || !el) return
  const wrapper = el.querySelector(`.code-block-wrapper[data-id="${id}"]`) as HTMLElement | null
  if (wrapper) removeAdjacentEmptyLines(wrapper)
}

function removeEmptyLinesAroundSelectedFoldBlock() {
  const id = selectedFoldBlockId.value
  const el = editorRef.value
  if (!id || !el) return
  const wrapper = el.querySelector(`.fold-block[data-id="${id}"]`) as HTMLElement | null
  if (wrapper) removeAdjacentEmptyLines(wrapper)
}

// --- Interaction Logic ---

function selectionChangeHandler() {
  if (!editorRef.value) return
  const sel = window.getSelection(); if (!sel || sel.rangeCount === 0) return
  const anchor = sel.anchorNode; if (!anchor || !editorRef.value.contains(anchor)) return
  let container: any = anchor; while (container && container.parentNode !== editorRef.value) container = container.parentNode
  if (container) {
    const kids = editorRef.value.childNodes; for (let i = 0; i < kids.length; i++) if (kids[i] === container) lastCursorBlockIndex.value = i
  }
}

function onEditorClick(e: MouseEvent) {
  const target = e.target as HTMLElement

  const codeWrapper = target.closest('.code-block-wrapper') as HTMLElement | null
  if (codeWrapper) {
    const id = codeWrapper.getAttribute('data-id')
    if (id) {
      selectedCodeBlockId.value = id
      const rect = codeWrapper.getBoundingClientRect()
      const top = rect.top - CODE_TOOLBAR_APPROX_HEIGHT - IMAGE_TOOLBAR_PADDING < 0 ? rect.bottom + window.scrollY + IMAGE_TOOLBAR_PADDING : rect.top + window.scrollY - CODE_TOOLBAR_APPROX_HEIGHT - IMAGE_TOOLBAR_PADDING
      let left = rect.left + rect.width / 2 - CODE_TOOLBAR_APPROX_WIDTH / 2 + window.scrollX
      left = Math.max(IMAGE_TOOLBAR_PADDING, Math.min(window.innerWidth - CODE_TOOLBAR_APPROX_WIDTH - IMAGE_TOOLBAR_PADDING + window.scrollX, left))
      selectedCodeBlockRect.value = { top, left }
    }
    selectedImageId.value = selectedFileId.value = selectedFoldBlockId.value = null
    return
  }

  const foldBlockStructure = target.closest('.fold-block') as HTMLElement | null
  if (foldBlockStructure) {
    const id = foldBlockStructure.getAttribute('data-id')
    if (id) {
      selectedFoldBlockId.value = id
      const rect = foldBlockStructure.getBoundingClientRect()
      const top = rect.top - FOLD_TOOLBAR_APPROX_HEIGHT - IMAGE_TOOLBAR_PADDING < 0 ? rect.bottom + window.scrollY + IMAGE_TOOLBAR_PADDING : rect.top + window.scrollY - FOLD_TOOLBAR_APPROX_HEIGHT - IMAGE_TOOLBAR_PADDING
      let left = rect.left + rect.width / 2 - FOLD_TOOLBAR_APPROX_WIDTH / 2 + window.scrollX
      left = Math.max(IMAGE_TOOLBAR_PADDING, Math.min(window.innerWidth - FOLD_TOOLBAR_APPROX_WIDTH - IMAGE_TOOLBAR_PADDING + window.scrollX, left))
      selectedFoldBlockRect.value = { top, left }
    }
    selectedImageId.value = selectedFileId.value = selectedCodeBlockId.value = null
    if (!target.closest('.fold-content')) {
      (foldBlockStructure.querySelector('.fold-content') as HTMLElement | null)?.focus()
    }
    return
  }

  const taskCheckbox = target.closest('.task-item-checkbox') as HTMLInputElement | null
  if (taskCheckbox) {
    e.preventDefault()
    const taskItem = taskCheckbox.closest('li.task-item')
    const id = taskItem?.getAttribute('data-id')
    const currentChecked = taskItem?.getAttribute('data-checked') === 'true'
    if (id && Array.isArray(props.modelValue)) {
      emit('update:modelValue', updateTaskItemCheckedInTree(props.modelValue, id, !currentChecked))
    }
    return
  }

  if (target.closest('.image-toolbar-root') || target.closest('.file-toolbar-root') || target.closest('.code-toolbar-root') || target.closest('.fold-toolbar-root')) return

  const img = target.closest('img.image-block') as HTMLElement | null || target.closest('.image-block-wrapper')?.querySelector('img.image-block') as HTMLElement | null
  if (img) {
    const id = img.getAttribute('data-id')
    if (id) {
      selectedImageId.value = id
      const rect = img.getBoundingClientRect()
      const top = rect.top - IMAGE_TOOLBAR_APPROX_HEIGHT - IMAGE_TOOLBAR_PADDING < 0 ? rect.bottom + window.scrollY + IMAGE_TOOLBAR_PADDING : rect.top + window.scrollY - IMAGE_TOOLBAR_APPROX_HEIGHT - IMAGE_TOOLBAR_PADDING
      let left = rect.left + rect.width / 2 - IMAGE_TOOLBAR_APPROX_WIDTH / 2 + window.scrollX
      left = Math.max(IMAGE_TOOLBAR_PADDING, Math.min(window.innerWidth - IMAGE_TOOLBAR_APPROX_WIDTH - IMAGE_TOOLBAR_PADDING + window.scrollX, left))
      selectedImageRect.value = { top, left }
    }
    selectedFileId.value = selectedCodeBlockId.value = selectedFoldBlockId.value = null
    return
  }

  const fileWrapper = target.closest('.file-block-wrapper') as HTMLElement | null
  if (fileWrapper) {
    const id = fileWrapper.getAttribute('data-id')
    if (id) {
      selectedFileId.value = id
      const block = fileWrapper.querySelector('.file-block') as HTMLElement | null || fileWrapper
      const rect = block.getBoundingClientRect()
      const top = rect.top - FILE_TOOLBAR_APPROX_HEIGHT - IMAGE_TOOLBAR_PADDING < 0 ? rect.bottom + window.scrollY + IMAGE_TOOLBAR_PADDING : rect.top + window.scrollY - FILE_TOOLBAR_APPROX_HEIGHT - IMAGE_TOOLBAR_PADDING
      let left = rect.left + rect.width / 2 - FILE_TOOLBAR_APPROX_WIDTH / 2 + window.scrollX
      left = Math.max(IMAGE_TOOLBAR_PADDING, Math.min(window.innerWidth - FILE_TOOLBAR_APPROX_WIDTH - IMAGE_TOOLBAR_PADDING + window.scrollX, left))
      selectedFileRect.value = { top, left }
    }
    selectedImageId.value = selectedCodeBlockId.value = selectedFoldBlockId.value = null
    return
  }

  selectedImageId.value = selectedFileId.value = selectedCodeBlockId.value = selectedFoldBlockId.value = null
}

function onEditorContextMenu(e: MouseEvent) {
  const target = e.target as HTMLElement
  if (target.closest('.image-toolbar-root')) return
  const img = target.closest('img.image-block') || target.closest('.image-block-wrapper')?.querySelector('img.image-block')
  if (img) {
    const assetPath = img.getAttribute('data-asset-path'); const id = img.getAttribute('data-id')
    if (assetPath) { e.preventDefault(); emit('contextmenu', { type: 'image', assetPath, id: id || '', clientX: e.clientX, clientY: e.clientY }) }
    return
  }
  const fileWrapper = target.closest('.file-block-wrapper')
  if (fileWrapper) {
    const assetPath = fileWrapper.getAttribute('data-asset-path'); const id = fileWrapper.getAttribute('data-id')
    if (assetPath) { e.preventDefault(); emit('contextmenu', { type: 'file', assetPath, id: id || '', clientX: e.clientX, clientY: e.clientY }) }
  }
}

function onEditorDblClick(e: MouseEvent) {
  const img = (e.target as HTMLElement).closest('img.image-block') as HTMLImageElement | null
  if (img) { const assetPath = img.getAttribute('data-asset-path'); const id = img.getAttribute('data-id'); if (assetPath && id) emit('open-asset', { type: 'image', assetPath, id }) }
}

function updateImageNode(updates: any) { if (selectedImageId.value && Array.isArray(props.modelValue)) emit('update:modelValue', updateNodeInTree(props.modelValue, selectedImageId.value, 'image', updates)) }
function updateFileNode(updates: any) { if (selectedFileId.value && Array.isArray(props.modelValue)) emit('update:modelValue', updateNodeInTree(props.modelValue, selectedFileId.value, 'file', updates)) }

// --- Commands ---

let savedRange: Range | null = null
const saveSelection = () => {
  const sel = window.getSelection()
  if (!editorRef.value || !sel || sel.rangeCount === 0) return
  const range = sel.getRangeAt(0)
  if (editorRef.value.contains(range.commonAncestorContainer)) savedRange = range.cloneRange()
  else savedRange = null
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
  if (command === 'formatBlock' && value) document.execCommand('formatBlock', false, value)
  else if (['justifyLeft', 'justifyCenter', 'justifyRight', 'undo', 'redo'].includes(command)) document.execCommand(command, false)
  else document.execCommand(command, false, value)
  handleInput()
}

/** 检查元素是否为空段落 */
function isEmptyP(el: Node | null): boolean {
  return el instanceof HTMLElement && (el.tagName.toLowerCase() === 'p' || el.tagName.startsWith('H')) && (el.innerHTML === '<br>' || el.innerText.trim() === '')
}

/** 获取前一个块元素（跳过 br 等） */
function getPreviousBlockElement(blockEl: HTMLElement): HTMLElement | null {
  let n: Node | null = blockEl.previousSibling
  while (n) {
    if (n.nodeType === Node.ELEMENT_NODE) {
      const el = n as HTMLElement
      if (el.classList?.contains('editor-block')) return el
      n = n.previousSibling
    } else n = n.previousSibling
  }
  return null
}

/** 获取后一个块元素 */
function getNextBlockElement(blockEl: HTMLElement): HTMLElement | null {
  let n: Node | null = blockEl.nextSibling
  while (n) {
    if (n.nodeType === Node.ELEMENT_NODE) {
      const el = n as HTMLElement
      if (el.classList?.contains('editor-block')) return el
      n = n.nextSibling
    } else n = n.nextSibling
  }
  return null
}

/** 移除指定块上方和下方的空行（空段落 + 紧跟的 br） */
function removeAdjacentEmptyLines(blockEl: HTMLElement) {
  let prev = getPreviousBlockElement(blockEl)
  while (prev && isEmptyP(prev)) {
    const brAfter = prev.nextSibling
    prev.remove()
    if (brAfter && brAfter.nodeType === Node.ELEMENT_NODE && (brAfter as HTMLElement).tagName === 'BR') brAfter.remove()
    prev = getPreviousBlockElement(blockEl)
  }
  let next = getNextBlockElement(blockEl)
  while (next && isEmptyP(next)) {
    const brAfter = next.nextSibling
    next.remove()
    if (brAfter && brAfter.nodeType === Node.ELEMENT_NODE && (brAfter as HTMLElement).tagName === 'BR') brAfter.remove()
    next = getNextBlockElement(blockEl)
  }
  handleInput()
}

function insertTaskListAtSelection() {
  restoreSelection(); const info = getInsertionContainer(); if (!info.container) return
  const currentBlock = info.container.childNodes[info.index] as HTMLElement
  let children: EditorNode[] = []
  if (currentBlock && (currentBlock.tagName.toLowerCase() === 'p' || currentBlock.tagName.startsWith('H'))) {
    const tempDiv = document.createElement('div'); tempDiv.innerHTML = currentBlock.innerHTML
    children = domToNodesFromContainer(tempDiv)
    if (children.length === 1 && children[0].type === 'text' && children[0].value === '') children = []
  }
  const taskNode: EditorNode = { type: 'taskList', id: genId(), children: [{ type: 'taskItem', id: genId(), checked: false, children }] }
  const wrap = document.createElement('div'); wrap.innerHTML = nodesToHtml([taskNode])
  const el = wrap.firstElementChild; if (!el) return
  if (currentBlock) info.container.replaceChild(el, currentBlock)
  else info.container.appendChild(el)
  handleInput()
  nextTick(() => postRender())
}

function insertCodeBlock() {
  insertNodesAtSelection({ type: 'code', id: genId(), content: '', language: 'text' })
}

function insertFoldBlock() {
  const sel = window.getSelection(); if (!sel || sel.rangeCount === 0 || !editorRef.value) return
  let depth = 0, curr: any = sel.anchorNode; while (curr && curr !== editorRef.value) { if (curr.classList?.contains('fold-block')) depth++; curr = curr.parentNode }
  if (depth >= 3) return
  insertNodesAtSelection({ type: 'fold', id: genId(), folded: false, children: [{ type: 'p', id: genId(), children: [] }] })
}

function insertNodesAtSelection(newNodes: EditorNode | EditorNode[]) {
  restoreSelection(); const info = getInsertionContainer(); if (!info.container) return
  const nodesArray = Array.isArray(newNodes) ? newNodes : [newNodes]
  const frag = document.createDocumentFragment(); nodesArray.forEach(n => {
    const wrap = document.createElement('div'); wrap.innerHTML = nodesToHtml([n]); if (wrap.firstElementChild) frag.appendChild(wrap.firstElementChild)
  })
  const currentBlock = info.container.childNodes[info.index]
  if (isEmptyP(currentBlock)) {
    info.container.replaceChild(frag, currentBlock)
  } else {
    if (info.index >= 0 && info.index < info.container.childNodes.length) info.container.insertBefore(frag, info.container.childNodes[info.index + 1] || null)
    else info.container.appendChild(frag)
  }
  handleInput()
  nextTick(() => postRender())
}


defineExpose({
  execCommand,
  handleInput,
  saveSelection,
  restoreSelection,
  getCursorBlockIndex: () => lastCursorBlockIndex.value,
  insertTaskListAtSelection,
  insertPlainText,
  insertCodeBlock,
  insertFoldBlock,
  insertNodesAtSelection
})
</script>

<template>
  <div class="advanced-editor-container">
    <div ref="editorRef" class="advanced-editor" contenteditable="true" @input="handleInput" @paste="handlePaste"
      @keydown.enter.capture="handleEnterKey" @beforeinput="handleBeforeInput" @click="onEditorClick"
      @dblclick="onEditorDblClick" @contextmenu="onEditorContextMenu" @keydown="handleKeyDown"
      @focusout="updateCodeHighlighting"></div>
    <Teleport to="body">
      <div v-if="selectedImageId && selectedImageNode" class="image-toolbar-root image-toolbar"
        :style="{ top: selectedImageRect.top + 'px', left: selectedImageRect.left + 'px' }">
        <label class="image-toolbar-label">宽度%</label>
        <input type="number" class="image-toolbar-input" min="10" max="100"
          :value="selectedImageNode.widthPercent ?? 100"
          @change="(e) => updateImageNode({ widthPercent: Math.min(100, Math.max(10, Number((e.target as HTMLInputElement).value) || 100)) })" />
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
      <div v-if="selectedCodeBlockId && selectedCodeBlockNode" class="code-toolbar-root code-toolbar"
        :style="{ top: selectedCodeBlockRect.top + 'px', left: selectedCodeBlockRect.left + 'px' }">
        <select class="code-toolbar-select" :value="selectedCodeBlockNode.language || 'text'"
          @change="(e) => setCodeBlockLanguage(selectedCodeBlockId!, (e.target as HTMLSelectElement).value)">
          <option v-for="opt in CODE_LANGUAGES" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
        </select>
        <button type="button" class="code-toolbar-btn" :title="t('editor.codeCopy')" @click="copyCodeBlockContent(selectedCodeBlockId!)">
          <Copy :size="16" />
        </button>
        <button type="button" class="code-toolbar-btn" :title="t('editor.foldInsertAfter')" @click="insertAfterSelectedCodeBlock">
          <Plus :size="16" />
        </button>
        <button type="button" class="code-toolbar-btn" title="移除上下空行" @click="removeEmptyLinesAroundSelectedCodeBlock">
          <Minus :size="16" />
        </button>
        <button type="button" class="code-toolbar-btn" title="删除块" @click="deleteBlockById(selectedCodeBlockId!)">
          <Trash2 :size="16" />
        </button>
      </div>
      <div v-if="selectedFoldBlockId && selectedFoldBlockNode" class="fold-toolbar-root fold-toolbar"
        :style="{ top: selectedFoldBlockRect.top + 'px', left: selectedFoldBlockRect.left + 'px' }">
        <button type="button" class="fold-toolbar-btn" :title="selectedFoldBlockNode.folded ? '展开' : '折叠'"
          @click="toggleFoldBlock(selectedFoldBlockId!)">
          <ChevronDown v-if="selectedFoldBlockNode.folded" :size="16" />
          <ChevronUp v-else :size="16" />
        </button>
        <button type="button" class="fold-toolbar-btn" :title="t('editor.foldInsertAfter')" @click="insertAfterSelectedFoldBlock">
          <Plus :size="16" />
        </button>
        <button type="button" class="fold-toolbar-btn" title="移除上下空行" @click="removeEmptyLinesAroundSelectedFoldBlock">
          <Minus :size="16" />
        </button>
        <button type="button" class="fold-toolbar-btn" title="删除块" @click="deleteBlockById(selectedFoldBlockId!)">
          <Trash2 :size="16" />
        </button>
      </div>
      <div v-if="selectedFileId && selectedFileNode" class="file-toolbar-root file-toolbar"
        :style="{ top: selectedFileRect.top + 'px', left: selectedFileRect.left + 'px' }">
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
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
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

/* 深色模式下代码块内容区域统一深色背景（覆盖 highlight.js 浅色主题带来的白色） */
.dark :deep(.code-editor),
.dark :deep(.code-editor.hljs),
.dark :deep(.code-editor .hljs) {
  background: #0d1117 !important;
  color: #c9d1d9 !important;
}
.dark :deep(.code-editor pre),
.dark :deep(.code-editor code) {
  background: transparent !important;
  color: inherit !important;
}
.dark :deep(.code-editor .hljs-doctag),
.dark :deep(.code-editor .hljs-keyword),
.dark :deep(.code-editor .hljs-meta .hljs-keyword),
.dark :deep(.code-editor .hljs-template-tag),
.dark :deep(.code-editor .hljs-template-variable),
.dark :deep(.code-editor .hljs-type),
.dark :deep(.code-editor .hljs-variable.language_) { color: #ff7b72; }
.dark :deep(.code-editor .hljs-title),
.dark :deep(.code-editor .hljs-title.class_),
.dark :deep(.code-editor .hljs-title.class_.inherited__),
.dark :deep(.code-editor .hljs-title.function_) { color: #d2a8ff; }
.dark :deep(.code-editor .hljs-attr),
.dark :deep(.code-editor .hljs-attribute),
.dark :deep(.code-editor .hljs-literal),
.dark :deep(.code-editor .hljs-meta),
.dark :deep(.code-editor .hljs-number),
.dark :deep(.code-editor .hljs-operator),
.dark :deep(.code-editor .hljs-variable),
.dark :deep(.code-editor .hljs-selector-attr),
.dark :deep(.code-editor .hljs-selector-class),
.dark :deep(.code-editor .hljs-selector-id) { color: #79c0ff; }
.dark :deep(.code-editor .hljs-regexp),
.dark :deep(.code-editor .hljs-string),
.dark :deep(.code-editor .hljs-meta .hljs-string) { color: #a5d6ff; }
.dark :deep(.code-editor .hljs-built_in),
.dark :deep(.code-editor .hljs-symbol) { color: #ffa657; }
.dark :deep(.code-editor .hljs-comment),
.dark :deep(.code-editor .hljs-code),
.dark :deep(.code-editor .hljs-formula) { color: #8b949e; }
.dark :deep(.code-editor .hljs-name),
.dark :deep(.code-editor .hljs-quote),
.dark :deep(.code-editor .hljs-selector-tag),
.dark :deep(.code-editor .hljs-selector-pseudo) { color: #7ee787; }
.dark :deep(.code-editor .hljs-subst) { color: #c9d1d9; }
.dark :deep(.code-editor .hljs-section) { color: #1f6feb; font-weight: bold; }
.dark :deep(.code-editor .hljs-bullet) { color: #f2cc60; }
.dark :deep(.code-editor .hljs-emphasis) { color: #c9d1d9; font-style: italic; }
.dark :deep(.code-editor .hljs-strong) { color: #c9d1d9; font-weight: bold; }
.dark :deep(.code-editor .hljs-addition) { color: #aff5b4; background-color: #033a16; }
.dark :deep(.code-editor .hljs-deletion) { color: #ffdcd7; background-color: #67060c; }

/* Fold Block Styles */
:deep(.fold-block) {
  margin: 20px 0;
  border: none;
  padding: 0;
}

:deep(.fold-line-top),
:deep(.fold-line-bottom) {
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

/* 代码块 / 折叠块悬浮工具栏（与图片/文件工具栏一致） */
.code-toolbar-root,
.fold-toolbar-root {
  position: absolute;
  z-index: 1000;
}

.code-toolbar,
.fold-toolbar {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 8px;
  background: var(--app-bg-color);
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  border: 1px solid rgba(0, 0, 0, 0.1);
}

.dark .code-toolbar,
.dark .fold-toolbar {
  background: var(--app-surface-color);
  border-color: rgba(255, 255, 255, 0.15);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
}

.code-toolbar-select {
  padding: 4px 8px;
  border-radius: 6px;
  border: 1px solid var(--app-border-color);
  background: var(--app-bg-color);
  color: var(--app-text-color);
  font-size: 12px;
  min-width: 100px;
}

.dark .code-toolbar-select {
  background: var(--app-surface-color);
}

.code-toolbar-btn,
.fold-toolbar-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  min-width: 28px;
  flex-shrink: 0;
  padding: 0;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--app-text-color);
  cursor: pointer;
  transition: background 0.2s;
}

.code-toolbar-btn svg,
.fold-toolbar-btn svg {
  width: 16px;
  height: 16px;
  color: inherit;
  stroke: currentColor;
}

.code-toolbar-btn:hover,
.fold-toolbar-btn:hover {
  background: rgba(0, 0, 0, 0.08);
}

.dark .code-toolbar-btn:hover,
.dark .fold-toolbar-btn:hover {
  background: rgba(255, 255, 255, 0.1);
}
</style>

<!-- 工具栏在 Teleport 到 body 时需全局样式，确保图标与按钮可见 -->
<style>
.code-toolbar-root .code-toolbar,
.fold-toolbar-root .fold-toolbar {
  color: var(--app-text-color);
}
.code-toolbar-root .code-toolbar-btn,
.fold-toolbar-root .fold-toolbar-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  min-width: 28px;
  flex-shrink: 0;
  padding: 0;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--app-text-color);
  cursor: pointer;
}
.code-toolbar-root .code-toolbar-btn svg,
.fold-toolbar-root .fold-toolbar-btn svg {
  width: 16px;
  height: 16px;
  flex-shrink: 0;
  color: currentColor;
  stroke: currentColor;
}
</style>
