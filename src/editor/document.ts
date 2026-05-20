import { normalizeCanvasDimension, normalizeCanvasMode, normalizeCanvasObjects, DEFAULT_CANVAS_HEIGHT, DEFAULT_CANVAS_WIDTH } from './canvas'
import type { EditorNode } from './types'

export function genEditorId(): string {
  return crypto.randomUUID()
}

export function cloneDocument(nodes: EditorNode[]): EditorNode[] {
  return structuredClone(nodes)
}

export function isContainerNode(node: EditorNode): node is EditorNode & { children: EditorNode[] } {
  return 'children' in node && Array.isArray(node.children)
}

export function isTextBlock(node: EditorNode): node is Extract<EditorNode, { type: 'p' | 'h1' | 'h2' | 'li' | 'taskItem' }> {
  return node.type === 'p' || node.type === 'h1' || node.type === 'h2' || node.type === 'li' || node.type === 'taskItem'
}

export function isRootBlock(node: EditorNode): boolean {
  return node.type !== 'text' && node.type !== 'strong' && node.type !== 'em' && node.type !== 'color' && node.type !== 'li' && node.type !== 'taskItem'
}

export function emptyParagraph(): EditorNode {
  return { type: 'p', id: genEditorId(), children: [] }
}

export function normalizeDocument(nodes: EditorNode[]): EditorNode[] {
  const normalized = nodes.map(normalizeNode).filter(Boolean) as EditorNode[]
  return normalized.length ? normalized : [emptyParagraph()]
}

export function normalizeNode(node: EditorNode): EditorNode | null {
  if (node.type === 'text') {
    return node.value.length ? { type: 'text', value: node.value } : null
  }

  if (node.type === 'strong' || node.type === 'em') {
    return { ...node, children: normalizeInlineChildren(node.children) }
  }

  if (node.type === 'color') {
    return { ...node, children: normalizeInlineChildren(node.children) }
  }

  if (node.type === 'p' || node.type === 'h1' || node.type === 'h2') {
    return {
      ...node,
      id: node.id || genEditorId(),
      children: normalizeInlineChildren(node.children),
      ...(node.align && node.align !== 'left' ? { align: node.align } : {})
    }
  }

  if (node.type === 'li') {
    return { ...node, id: node.id || genEditorId(), children: normalizeInlineChildren(node.children) }
  }

  if (node.type === 'taskItem') {
    return { ...node, id: node.id || genEditorId(), checked: node.checked === true, children: normalizeInlineChildren(node.children) }
  }

  if (node.type === 'ul' || node.type === 'ol') {
    const children = node.children
      .map((child) => normalizeNode(child))
      .filter((child): child is EditorNode => !!child && child.type === 'li')
    return { ...node, id: node.id || genEditorId(), children: children.length ? children : [{ type: 'li', id: genEditorId(), children: [] }] }
  }

  if (node.type === 'taskList') {
    const children = node.children
      .map((child) => normalizeNode(child))
      .filter((child): child is EditorNode => !!child && child.type === 'taskItem')
    return { ...node, id: node.id || genEditorId(), children: children.length ? children : [{ type: 'taskItem', id: genEditorId(), checked: false, children: [] }] }
  }

  if (node.type === 'fold') {
    return { ...node, id: node.id || genEditorId(), folded: node.folded === true, children: normalizeDocument(node.children) }
  }

  if (node.type === 'image') {
    return {
      ...node,
      id: node.id || genEditorId(),
      widthPercent: node.widthPercent == null ? 100 : Math.min(100, Math.max(10, node.widthPercent)),
      align: node.align || 'left'
    }
  }

  if (node.type === 'file') {
    return { ...node, id: node.id || genEditorId(), ...(node.align && node.align !== 'left' ? { align: node.align } : {}) }
  }

  if (node.type === 'code') {
    return { ...node, id: node.id || genEditorId(), content: node.content ?? '', language: node.language || 'text' }
  }

  if (node.type === 'markdown') {
    const mode = node.mode === 'preview' || node.mode === 'split' ? node.mode : 'edit'
    return { ...node, id: node.id || genEditorId(), content: node.content ?? '', mode }
  }

  if (node.type === 'canvas') {
    return {
      ...node,
      id: node.id || genEditorId(),
      width: normalizeCanvasDimension(node.width, DEFAULT_CANVAS_WIDTH),
      height: normalizeCanvasDimension(node.height, DEFAULT_CANVAS_HEIGHT),
      mode: normalizeCanvasMode(node.mode),
      objects: normalizeCanvasObjects(node.objects)
    }
  }

  return node
}

function normalizeInlineChildren(children: EditorNode[]): EditorNode[] {
  const normalized = children.map(normalizeNode).filter(Boolean) as EditorNode[]
  const merged: EditorNode[] = []

  for (const child of normalized) {
    const prev = merged[merged.length - 1]
    if (prev?.type === 'text' && child.type === 'text') {
      prev.value += child.value
      continue
    }
    if (prev?.type === 'strong' && child.type === 'strong') {
      prev.children = normalizeInlineChildren([...prev.children, ...child.children])
      continue
    }
    if (prev?.type === 'em' && child.type === 'em') {
      prev.children = normalizeInlineChildren([...prev.children, ...child.children])
      continue
    }
    if (prev?.type === 'color' && child.type === 'color' && prev.color === child.color) {
      prev.children = normalizeInlineChildren([...prev.children, ...child.children])
      continue
    }
    merged.push(child)
  }

  return merged
}

export function stripFileSizes(nodes: EditorNode[]): EditorNode[] {
  return nodes.map((node) => {
    if (node.type === 'file') {
      const { fileSize: _, ...rest } = node
      return rest as EditorNode
    }
    if (node.type === 'canvas') {
      return {
        ...node,
        objects: node.objects.map((object) => {
          if (object.kind !== 'file') return object
          const { fileSize: _, ...rest } = object
          return rest
        })
      } as EditorNode
    }
    if (isContainerNode(node)) {
      return { ...node, children: stripFileSizes(node.children) } as EditorNode
    }
    return node
  })
}

export function stableDocumentJson(nodes: EditorNode[]): string {
  return JSON.stringify(stripFileSizes(normalizeDocument(nodes)))
}

export function getTextFromEditorNode(node: EditorNode): string {
  if (node.type === 'text') return node.value ?? ''
  if (isContainerNode(node)) return node.children.map(getTextFromEditorNode).join('')
  if (node.type === 'code') return node.content
  if (node.type === 'markdown') return node.content
  if (node.type === 'canvas') {
    return node.objects
      .map((object) => object.kind === 'text' ? object.text : object.kind === 'file' ? object.fileName || '' : '')
      .filter(Boolean)
      .join(' ')
  }
  return ''
}

export function collectTaskItems(nodes: EditorNode[], emptyLabel = '(无文字)'): { node: Extract<EditorNode, { type: 'taskItem' }>; label: string }[] {
  const out: { node: Extract<EditorNode, { type: 'taskItem' }>; label: string }[] = []
  function walk(items: EditorNode[]) {
    for (const node of items) {
      if (node.type === 'taskItem') {
        out.push({ node, label: getTextFromEditorNode(node).trim() || emptyLabel })
      }
      if (isContainerNode(node)) walk(node.children)
    }
  }
  walk(nodes)
  return out
}

export function countTaskProgress(nodes: unknown): { done: number; total: number } {
  let done = 0
  let total = 0
  function walk(obj: unknown) {
    if (!obj || typeof obj !== 'object') return
    if (Array.isArray(obj)) {
      obj.forEach(walk)
      return
    }
    const node = obj as { type?: string; checked?: boolean; children?: unknown[] }
    if (node.type === 'taskItem') {
      total += 1
      if (node.checked === true) done += 1
    }
    if (Array.isArray(node.children)) node.children.forEach(walk)
  }
  walk(nodes)
  return { done, total }
}

export function collectUsedAssetNames(nodes: EditorNode[]): Set<string> {
  const names = new Set<string>()
  function addFromPath(path: string | undefined) {
    if (!path) return
    const name = path.split(/[/\\]/).pop()
    if (name) names.add(name)
  }
  function addAsset(assetPath?: string, url?: string) {
    addFromPath(assetPath)
    if (!url) return
    try {
      addFromPath(decodeURIComponent(url))
    } catch (_) {
      addFromPath(url)
    }
  }
  function walk(items: EditorNode[]) {
    for (const node of items) {
      if (node.type === 'image' || node.type === 'file') {
        addAsset(node.assetPath, node.url)
      }
      if (node.type === 'canvas') {
        node.objects.forEach((object) => {
          if (object.kind === 'image' || object.kind === 'file') addAsset(object.assetPath, object.url)
        })
      }
      if (isContainerNode(node)) walk(node.children)
    }
  }
  walk(nodes)
  return names
}

export function findNodeById<T extends EditorNode['type']>(nodes: EditorNode[], id: string, type?: T): Extract<EditorNode, { type: T }> | EditorNode | null {
  for (const node of nodes) {
    if ('id' in node && node.id === id && (!type || node.type === type)) return node
    if (isContainerNode(node)) {
      const found = findNodeById(node.children, id, type)
      if (found) return found
    }
  }
  return null
}

export function findPathById(nodes: EditorNode[], id: string): number[] | null {
  function walk(items: EditorNode[], basePath: number[]): number[] | null {
    for (let index = 0; index < items.length; index += 1) {
      const node = items[index]
      const path = [...basePath, index]
      if ('id' in node && node.id === id) return path
      if (isContainerNode(node)) {
        const found = walk(node.children, path)
        if (found) return found
      }
    }
    return null
  }
  return walk(nodes, [])
}

export function updateNodeById(nodes: EditorNode[], id: string, updates: Partial<EditorNode>): EditorNode[] {
  return nodes.map((node) => {
    if ('id' in node && node.id === id) return { ...node, ...updates } as EditorNode
    if (isContainerNode(node)) return { ...node, children: updateNodeById(node.children, id, updates) } as EditorNode
    return node
  })
}

export function deleteNodeById(nodes: EditorNode[], id: string): EditorNode[] {
  const next: EditorNode[] = []
  for (const node of nodes) {
    if ('id' in node && node.id === id) continue
    if (isContainerNode(node)) next.push({ ...node, children: deleteNodeById(node.children, id) } as EditorNode)
    else next.push(node)
  }
  return normalizeDocument(next)
}

export function insertAfterNodeById(nodes: EditorNode[], id: string, newNode: EditorNode): EditorNode[] {
  const next: EditorNode[] = []
  let inserted = false
  for (const node of nodes) {
    if (inserted) {
      next.push(node)
      continue
    }
    if ('id' in node && node.id === id) {
      next.push(node, newNode)
      inserted = true
      continue
    }
    if (isContainerNode(node)) {
      const children = insertAfterNodeById(node.children, id, newNode)
      if (children !== node.children) {
        next.push({ ...node, children } as EditorNode)
        inserted = true
        continue
      }
    }
    next.push(node)
  }
  return inserted ? next : nodes
}

export function removeAdjacentEmptyParagraphsById(nodes: EditorNode[], id: string): EditorNode[] {
  const index = nodes.findIndex((node) => 'id' in node && node.id === id)
  if (index >= 0) {
    const next = nodes.filter((node, i) => {
      if ((i === index - 1 || i === index + 1) && node.type === 'p' && getTextFromEditorNode(node).trim() === '') return false
      return true
    })
    return normalizeDocument(next)
  }
  return nodes.map((node) => isContainerNode(node) ? { ...node, children: removeAdjacentEmptyParagraphsById(node.children, id) } as EditorNode : node)
}

export function foldDepthAtPath(nodes: EditorNode[], path: number[]): number {
  let depth = 0
  let cursor: EditorNode[] = nodes
  for (const index of path) {
    const node = cursor[index]
    if (!node) break
    if (node.type === 'fold') depth += 1
    if (!isContainerNode(node)) break
    cursor = node.children
  }
  return depth
}

export function insertNodesAtRoot(nodes: EditorNode[], index: number, newNodes: EditorNode[]): EditorNode[] {
  const normalized = normalizeDocument(nodes)
  const safeIndex = Math.max(0, Math.min(normalized.length, index))
  const next = [...normalized]
  next.splice(safeIndex, 0, ...newNodes)
  return normalizeDocument(next)
}