<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { AlignCenter, AlignLeft, AlignRight } from 'lucide-vue-next';
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';

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

const props = defineProps<{
  modelValue: EditorNode[]
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: EditorNode[]): void
  (e: 'upload-image'): void
  (e: 'upload-file'): void
  (e: 'contextmenu', payload: { type: 'image' | 'file'; assetPath: string; id: string; clientX: number; clientY: number }): void
  (e: 'open-asset', payload: { type: 'image' | 'file'; assetPath: string; id: string }): void
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
      const name = child.querySelector('.file-name')?.textContent ?? ''
      const url = child.getAttribute('data-url') ?? ''
      const ap = child.getAttribute('data-asset-path') ?? ''
      out.push({ type: 'file', id, url, fileName: name, assetPath: ap || undefined })
    } else if (child.classList.contains('file-block-wrapper')) {
      const fileBlock = child.querySelector('.file-block') as HTMLElement | null
      const name = fileBlock?.querySelector('.file-name')?.textContent ?? ''
      const url = child.getAttribute('data-url') ?? ''
      const ap = child.getAttribute('data-asset-path') ?? ''
      const align = getBlockAlign(child)
      out.push({ type: 'file', id, url, fileName: name, assetPath: ap || undefined, ...(align !== 'left' ? { align } : {}) })
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
      // 兼容旧格式（没有 wrapper）
      const fileCard = child.querySelector('.file-card')
      const name = child.querySelector('.file-name')?.textContent ?? ''
      const url = child.getAttribute('data-url') ?? ''
      const ap = child.getAttribute('data-asset-path') ?? ''
      roots.push({ type: 'file', id, url, fileName: name, assetPath: ap || undefined })
      if (fileCard) {
        roots.push(...collectTrailingBlocksFromFileBlock(child, fileCard))
      }
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
      roots.push(...(fromDiv.length > 0 ? fromDiv : [{ type: 'p' as const, id: genId(), children: [] }]))
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
          roots.push(...collectTrailingBlocksFromWrapper(el, img))
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
      } else if (el.classList.contains('file-block-wrapper')) {
        const fileBlock = el.querySelector('.file-block') as HTMLElement | null
        const fileCard = fileBlock?.querySelector('.file-card') as HTMLElement | null
        const name = fileBlock?.querySelector('.file-name')?.textContent ?? ''
        const url = el.getAttribute('data-url') ?? ''
        const ap = el.getAttribute('data-asset-path') ?? ''
        const align = getBlockAlign(el)
        roots.push({ type: 'file', id, url, fileName: name, assetPath: ap || undefined, ...(align !== 'left' ? { align } : {}) })
        if (fileCard) {
          roots.push(...collectTrailingBlocksFromFileBlock(el, fileCard))
        }
      } else if (el.classList.contains('file-block')) {
        // 兼容旧格式（没有 wrapper）
        const fileCard = el.querySelector('.file-card')
        const name = el.querySelector('.file-name')?.textContent ?? ''
        const ap = el.getAttribute('data-asset-path') ?? ''
        roots.push({ type: 'file', id, url: el.getAttribute('data-url') ?? '', fileName: name, assetPath: ap || undefined })
        if (fileCard) {
          roots.push(...collectTrailingBlocksFromFileBlock(el, fileCard))
        }
      } else if (tag === 'p' || tag === 'h1' || tag === 'h2') {
        const align = getBlockAlign(el)
        roots.push({ type: tag as 'p' | 'h1' | 'h2', id, ...(align !== 'left' ? { align } : {}), children: collectChildren(el) })
      } else if (el.classList.contains('task-list')) {
        const taskListChildren: EditorNode[] = []
        el.querySelectorAll(':scope > li.task-item').forEach(liEl => {
          const li = liEl as HTMLElement
          const contentEl = li.querySelector('.task-item-content') as HTMLElement | null
          const children = contentEl ? collectChildren(contentEl) : []
          const checked = li.getAttribute('data-checked') === 'true' || (li.querySelector('.task-item-checkbox') as HTMLInputElement | null)?.checked === true
          taskListChildren.push({ type: 'taskItem', id: li.getAttribute('data-id') ?? genId(), checked, children })
        })
        if (taskListChildren.length === 0) taskListChildren.push({ type: 'taskItem', id: genId(), checked: false, children: [] })
        roots.push({ type: 'taskList', id: el.getAttribute('data-id') ?? genId(), children: taskListChildren })
      } else if (tag === 'ul' || tag === 'ol') {
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

/** 收集图片 wrapper 内 img 之后的兄弟节点并序列化为块（解决在图片后直接输入时内容被丢的问题） */
function collectTrailingBlocksFromWrapper(wrapper: HTMLElement, img: Element): EditorNode[] {
  const temp = document.createElement('div')
  let next: ChildNode | null = img.nextSibling
  while (next) {
    // 跳过 trailing br（只是占位符，不应该被保存）
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
    // 跳过 trailing br（只是占位符，不应该被保存）
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

/** 阻止在图片/文件块内部输入 */
function handleBeforeInput(e: Event) {
  const inputEvent = e as InputEvent
  const selection = window.getSelection()
  if (!selection || selection.rangeCount === 0) return
  
  const range = selection.getRangeAt(0)
  const container = range.commonAncestorContainer
  const containerEl = container.nodeType === Node.ELEMENT_NODE ? container as HTMLElement : container.parentElement as HTMLElement | null
  const wrapper = containerEl?.closest('.image-block-wrapper, .file-block-wrapper, .file-block') as HTMLElement | null
  
  if (!wrapper) return
  
  const img = wrapper.querySelector('img.image-block')
  const fileCard = wrapper.querySelector('.file-card')
  const trailingBr = wrapper.querySelector('.asset-trailing-br')
  
  // 检查光标是否在图片/文件本身（img 或 file-card）内
  const isInAsset = img?.contains(container) || fileCard?.contains(container)
  
  // 如果光标在图片/文件本身内，阻止输入
  if (isInAsset) {
    e.preventDefault()
    return
  }
  
  // 如果光标在 trailing br 处或 wrapper 末尾（图片/文件后面），允许换行由 handleEnterKey 处理
  const isAtTrailingBrOrEnd = (trailingBr && (container === trailingBr || container.parentElement === trailingBr.parentElement)) ||
    (container === wrapper && range.startOffset >= (wrapper.childNodes.length || 1))
  if (isAtTrailingBrOrEnd) {
    if (inputEvent.inputType === 'insertLineBreak' || (inputEvent.inputType === 'insertText' && inputEvent.data === '\n')) {
      // 允许换行，handleEnterKey 会处理（在 keydown 里插入新段落）
      return
    }
    // 其他输入：阻止默认，插入新段落到 wrapper 后，移动光标并插入该字符
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
  
  // 如果光标在 wrapper 内但不在图片/文件本身和 trailing br，阻止输入
  if (wrapper.contains(container) && !isInAsset && container !== trailingBr && container.parentElement !== trailingBr?.parentElement) {
    e.preventDefault()
    // 将光标移到 wrapper 后面
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

/** 处理 Enter 键：在图片/文件后面时插入新段落 */
function handleEnterKey(e: KeyboardEvent) {
  const selection = window.getSelection()
  if (!selection || selection.rangeCount === 0 || !editorRef.value) return
  
  const range = selection.getRangeAt(0)
  const container = range.commonAncestorContainer
  const containerEl = container.nodeType === Node.ELEMENT_NODE ? container as HTMLElement : container.parentElement as HTMLElement | null
  const wrapper = containerEl?.closest('.image-block-wrapper, .file-block-wrapper, .file-block') as HTMLElement | null
  
  if (!wrapper) return
  
  const img = wrapper.querySelector('img.image-block')
  const fileCard = wrapper.querySelector('.file-card')
  const trailingBr = wrapper.querySelector('.asset-trailing-br')
  
  // 光标在图片/文件本身内时不处理（让浏览器默认行为或不做）
  const isInAsset = img?.contains(container) || fileCard?.contains(container)
  if (isInAsset) return
  
  // 光标在 wrapper 内（含 wrapper 自身、trailing br、或 wrapper 末尾）即视为“在图片/文件后面”
  const isInsideWrapper = wrapper === container || wrapper.contains(container)
  const isAtEndOfWrapper = container === wrapper && range.startOffset >= wrapper.childNodes.length
  const isAtTrailingBr = trailingBr && (container === trailingBr || container.parentElement === trailingBr.parentElement || range.startContainer === trailingBr)
  const isAfterAsset = isInsideWrapper && (isAtTrailingBr || isAtEndOfWrapper || (container === wrapper && range.startOffset > 0))
  
  // 或者光标在 wrapper 的父元素中，紧跟在 wrapper 后面
  const wrapperParent = wrapper.parentElement
  const containerParent = container.parentElement
  const isRightAfterWrapper = wrapperParent && containerParent && wrapperParent === containerParent && 
    container.nodeType === Node.TEXT_NODE && 
    Array.from(wrapperParent.childNodes).indexOf(wrapper) + 1 === Array.from(wrapperParent.childNodes).indexOf(container as ChildNode)
  
  if (isAfterAsset || isRightAfterWrapper) {
    e.preventDefault()
    
    // 创建新段落并插入到 wrapper 后面
    const newP = document.createElement('p')
    newP.className = 'editor-block'
    newP.setAttribute('data-id', genId())
    const br = document.createElement('br')
    newP.appendChild(br)
    
    // 插入新段落
    const wrapperParent = wrapper.parentElement
    if (wrapperParent) {
      if (wrapper.nextSibling) {
        wrapperParent.insertBefore(newP, wrapper.nextSibling)
      } else {
        wrapperParent.appendChild(newP)
      }
    }
    
    // 移动光标到新段落
    const newRange = document.createRange()
    newRange.setStart(newP, 0)
    newRange.collapse(true)
    selection.removeAllRanges()
    selection.addRange(newRange)
    
    // 触发更新
    handleInput()
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

onMounted(() => {
  if (!editorRef.value) return
  const nodes = Array.isArray(props.modelValue) && props.modelValue.length > 0 ? props.modelValue : defaultNodes()
  editorRef.value.innerHTML = nodesToHtml(nodes)
  nextTick(updateFileIcons)
})

watch(() => props.modelValue, (newVal) => {
  if (!isInternalUpdate.value && editorRef.value) {
    const nodes = Array.isArray(newVal) && newVal.length > 0 ? newVal : defaultNodes()
    editorRef.value.innerHTML = nodesToHtml(nodes)
    nextTick(updateFileIcons)
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
  } else if (command === 'undo') {
    document.execCommand('undo', false)
  } else if (command === 'redo') {
    document.execCommand('redo', false)
  } else {
    document.execCommand(command, false, value)
  }
  handleInput()
}

const selectedImageNode = computed(() => {
  if (!selectedImageId.value || !Array.isArray(props.modelValue)) return null
  return props.modelValue.find((n): n is EditorNode & { type: 'image' } => n.type === 'image' && (n as { id?: string }).id === selectedImageId.value) ?? null
})

const selectedFileNode = computed(() => {
  if (!selectedFileId.value || !Array.isArray(props.modelValue)) return null
  return props.modelValue.find((n): n is EditorNode & { type: 'file' } => n.type === 'file' && (n as { id?: string }).id === selectedFileId.value) ?? null
})

/** 在节点树中按 id 查找 taskItem */
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

/** 在节点树中把指定 id 的 taskItem 的 checked 设为给定值，返回新树 */
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
    nextTick(updateFileIcons)
    setTimeout(() => { isInternalUpdate.value = false }, 0)
    return
  }
  if (target.closest('.image-toolbar-root') || target.closest('.file-toolbar-root')) return
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
    selectedFileId.value = null
    return
  }
  if (target.classList.contains('file-block') || target.closest('.file-block-wrapper') || target.closest('.file-card')) {
    const wrapper = target.closest('.file-block-wrapper') as HTMLElement | null
    const fileBlock = wrapper?.querySelector('.file-block') as HTMLElement | null
    const id = wrapper?.getAttribute('data-id') ?? fileBlock?.getAttribute('data-id') ?? null
    if (id) {
      selectedFileId.value = id
      const rect = (fileBlock ?? wrapper)?.getBoundingClientRect()
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
    selectedImageId.value = null
    return
  }
  selectedImageId.value = null
  selectedFileId.value = null
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
  isInternalUpdate.value = true
  if (editorRef.value) editorRef.value.innerHTML = nodesToHtml(next)
  nextTick(updateFileIcons)
  setTimeout(() => { isInternalUpdate.value = false }, 0)
}

function updateFileNode(updates: { align?: 'left' | 'center' | 'right' }) {
  if (!selectedFileId.value || !Array.isArray(props.modelValue)) return
  const next = props.modelValue.map(node => {
    if (node.type !== 'file' || node.id !== selectedFileId.value) return node
    return { ...node, ...updates }
  })
  emit('update:modelValue', next)
  isInternalUpdate.value = true
  if (editorRef.value) editorRef.value.innerHTML = nodesToHtml(next)
  nextTick(updateFileIcons)
  setTimeout(() => { isInternalUpdate.value = false }, 0)
}

/** 处理双击：图片双击打开 */
function onEditorDblClick(e: MouseEvent) {
  const target = e.target as HTMLElement
  if (target.classList.contains('image-block') || target.closest('.image-block-wrapper')) {
    const wrapper = target.closest('.image-block-wrapper') as HTMLElement | null
    const img = (wrapper?.querySelector('img.image-block') ?? target) as HTMLImageElement
    const assetPath = img?.getAttribute('data-asset-path') ?? ''
    const id = img?.getAttribute('data-id') ?? ''
    if (assetPath && id) {
      emit('open-asset', { type: 'image', assetPath, id })
    }
  }
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

/** 在当前光标所在块之后插入一个任务列表（与“列表”按钮的插入位置逻辑一致） */
function insertTaskListAtSelection() {
  restoreSelection()
  const idx = getCursorBlockIndex()
  const taskNode: EditorNode = {
    type: 'taskList',
    id: genId(),
    children: [{ type: 'taskItem', id: genId(), checked: false, children: [] }]
  }
  const html = nodeToHtml(taskNode)
  const wrap = document.createElement('div')
  wrap.innerHTML = html
  const ul = wrap.firstElementChild
  if (!ul || !editorRef.value) return
  const editor = editorRef.value
  if (idx >= 0 && idx < editor.childNodes.length) {
    const next = editor.childNodes[idx + 1] || null
    editor.insertBefore(ul, next)
  } else {
    editor.appendChild(ul)
  }
  handleInput()
}

defineExpose({ execCommand, handleInput, saveSelection, restoreSelection, getCursorBlockIndex, insertTaskListAtSelection })
</script>

<template>
  <div class="advanced-editor-container">
    <div
      ref="editorRef"
      class="advanced-editor"
      contenteditable="true"
      @input="handleInput"
      @keydown.enter.capture="handleEnterKey"
      @beforeinput="handleBeforeInput"
      @click="onEditorClick"
      @dblclick="onEditorDblClick"
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
