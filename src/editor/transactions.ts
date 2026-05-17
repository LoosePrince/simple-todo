import type { EditorNode, InlineMarks, TextSegment } from './types'
import { cloneDocument, emptyParagraph, genEditorId, isContainerNode, isTextBlock, normalizeDocument } from './document'

export type EditorPath = number[]

export interface TextPosition {
  blockPath: EditorPath
  offset: number
}

export interface TextRange {
  anchor: TextPosition
  focus: TextPosition
}

export function getNodeAtPath(nodes: EditorNode[], path: EditorPath): EditorNode | null {
  let current: EditorNode[] = nodes
  let node: EditorNode | null = null
  for (const index of path) {
    node = current[index] ?? null
    if (!node) return null
    if (index !== path[path.length - 1]) {
      if (!isContainerNode(node)) return null
      current = node.children
    }
  }
  return node
}

export function updateNodeAtPath(nodes: EditorNode[], path: EditorPath, updater: (node: EditorNode) => EditorNode): EditorNode[] {
  if (!path.length) return nodes
  const [index, ...rest] = path
  return nodes.map((node, i) => {
    if (i !== index) return node
    if (!rest.length) return updater(node)
    if (!isContainerNode(node)) return node
    return { ...node, children: updateNodeAtPath(node.children, rest, updater) } as EditorNode
  })
}

export function insertNodesAfterPath(nodes: EditorNode[], path: EditorPath, newNodes: EditorNode[]): EditorNode[] {
  if (!path.length) return [...newNodes, ...nodes]
  const [index, ...rest] = path
  if (!rest.length) {
    const next = [...nodes]
    next.splice(index + 1, 0, ...newNodes)
    return normalizeDocument(next)
  }
  return nodes.map((node, i) => {
    if (i !== index || !isContainerNode(node)) return node
    return { ...node, children: insertNodesAfterPath(node.children, rest, newNodes) } as EditorNode
  })
}

export function insertNodesBeforePath(nodes: EditorNode[], path: EditorPath, newNodes: EditorNode[]): EditorNode[] {
  if (!path.length) return [...newNodes, ...nodes]
  const [index, ...rest] = path
  if (!rest.length) {
    const next = [...nodes]
    next.splice(index, 0, ...newNodes)
    return normalizeDocument(next)
  }
  return nodes.map((node, i) => {
    if (i !== index || !isContainerNode(node)) return node
    return { ...node, children: insertNodesBeforePath(node.children, rest, newNodes) } as EditorNode
  })
}

export function replaceNodeAtPathWithNodes(source: EditorNode[], path: EditorPath, replacements: EditorNode[]): EditorNode[] {
  if (!path.length) return source
  const [index, ...rest] = path
  if (!rest.length) {
    const next = [...source]
    next.splice(index, 1, ...replacements)
    return normalizeDocument(next)
  }
  return source.map((node, i) => {
    if (i !== index || !isContainerNode(node)) return node
    return { ...node, children: replaceNodeAtPathWithNodes(node.children, rest, replacements) } as EditorNode
  })
}

export function removeNodeAtPath(nodes: EditorNode[], path: EditorPath): EditorNode[] {
  if (!path.length) return nodes
  const [index, ...rest] = path
  if (!rest.length) {
    const next = [...nodes]
    next.splice(index, 1)
    return normalizeDocument(next)
  }
  return nodes.map((node, i) => {
    if (i !== index || !isContainerNode(node)) return node
    return { ...node, children: removeNodeAtPath(node.children, rest) } as EditorNode
  })
}

export function flattenInlineText(children: EditorNode[], inherited: InlineMarks = {}): TextSegment[] {
  const out: TextSegment[] = []
  for (const child of children) {
    if (child.type === 'text') {
      out.push({ text: child.value, marks: { ...inherited } })
    } else if (child.type === 'strong') {
      out.push(...flattenInlineText(child.children, { ...inherited, strong: true }))
    } else if (child.type === 'em') {
      out.push(...flattenInlineText(child.children, { ...inherited, em: true }))
    } else if (child.type === 'color') {
      out.push(...flattenInlineText(child.children, { ...inherited, color: child.color }))
    }
  }
  return mergeTextSegments(out)
}

export function inlineTextLength(children: EditorNode[]): number {
  return flattenInlineText(children).reduce((sum, seg) => sum + seg.text.length, 0)
}

export function inlineTextValue(children: EditorNode[]): string {
  return flattenInlineText(children).map((seg) => seg.text).join('')
}

export function segmentsToInlineNodes(segments: TextSegment[]): EditorNode[] {
  const nodes: EditorNode[] = []
  for (const segment of mergeTextSegments(segments)) {
    if (!segment.text) continue
    let node: EditorNode = { type: 'text', value: segment.text }
    if (segment.marks.color) node = { type: 'color', color: segment.marks.color, children: [node] }
    if (segment.marks.em) node = { type: 'em', children: [node] }
    if (segment.marks.strong) node = { type: 'strong', children: [node] }
    nodes.push(node)
  }
  return nodes
}

export function mergeTextSegments(segments: TextSegment[]): TextSegment[] {
  const out: TextSegment[] = []
  for (const seg of segments) {
    if (!seg.text) continue
    const prev = out[out.length - 1]
    if (prev && sameMarks(prev.marks, seg.marks)) {
      prev.text += seg.text
    } else {
      out.push({ text: seg.text, marks: { ...seg.marks } })
    }
  }
  return out
}

function sameMarks(a: InlineMarks, b: InlineMarks): boolean {
  return !!a.strong === !!b.strong && !!a.em === !!b.em && (a.color || '') === (b.color || '')
}

export function splitSegmentsAt(segments: TextSegment[], offset: number): [TextSegment[], TextSegment[]] {
  const left: TextSegment[] = []
  const right: TextSegment[] = []
  let cursor = 0
  for (const seg of segments) {
    const end = cursor + seg.text.length
    if (end <= offset) {
      left.push({ text: seg.text, marks: { ...seg.marks } })
    } else if (cursor >= offset) {
      right.push({ text: seg.text, marks: { ...seg.marks } })
    } else {
      const split = offset - cursor
      left.push({ text: seg.text.slice(0, split), marks: { ...seg.marks } })
      right.push({ text: seg.text.slice(split), marks: { ...seg.marks } })
    }
    cursor = end
  }
  return [mergeTextSegments(left), mergeTextSegments(right)]
}

export function insertTextIntoBlock(nodes: EditorNode[], path: EditorPath, offset: number, text: string, marks: InlineMarks = {}): EditorNode[] {
  return updateNodeAtPath(nodes, path, (node) => {
    if (!('children' in node) || !Array.isArray(node.children)) return node
    const segments = flattenInlineText(node.children)
    const [left, right] = splitSegmentsAt(segments, offset)
    return { ...node, children: segmentsToInlineNodes([...left, { text, marks }, ...right]) } as EditorNode
  })
}

export function replaceTextInBlock(nodes: EditorNode[], path: EditorPath, value: string): EditorNode[] {
  return updateNodeAtPath(nodes, path, (node) => {
    if (!('children' in node) || !Array.isArray(node.children)) return node
    return { ...node, children: value ? [{ type: 'text', value }] : [] } as EditorNode
  })
}

export function splitTextBlock(nodes: EditorNode[], path: EditorPath, offset: number): EditorNode[] {
  const node = getNodeAtPath(nodes, path)
  if (!node || !('children' in node) || !Array.isArray(node.children)) return nodes
  const segments = flattenInlineText(node.children)
  const [left, right] = splitSegmentsAt(segments, offset)
  const current = { ...node, children: segmentsToInlineNodes(left) } as EditorNode
  const next = createSiblingTextBlock(node, segmentsToInlineNodes(right))
  const withCurrent = updateNodeAtPath(nodes, path, () => current)
  return insertNodesAfterPath(withCurrent, path, [next])
}

function createSiblingTextBlock(node: EditorNode, children: EditorNode[]): EditorNode {
  if (node.type === 'h1' || node.type === 'h2' || node.type === 'p') {
    return { type: 'p', id: genEditorId(), children }
  }
  if (node.type === 'li') return { type: 'li', id: genEditorId(), children }
  if (node.type === 'taskItem') return { type: 'taskItem', id: genEditorId(), checked: false, children }
  return { type: 'p', id: genEditorId(), children }
}

export function setBlockText(nodes: EditorNode[], path: EditorPath, value: string): EditorNode[] {
  return normalizeDocument(replaceTextInBlock(cloneDocument(nodes), path, value))
}

export function setCodeContent(nodes: EditorNode[], id: string, content: string): EditorNode[] {
  function walk(items: EditorNode[]): EditorNode[] {
    return items.map((node) => {
      if (node.type === 'code' && node.id === id) return { ...node, content }
      if (isContainerNode(node)) return { ...node, children: walk(node.children) } as EditorNode
      return node
    })
  }
  return walk(nodes)
}

export function createParagraphFromText(text = ''): EditorNode {
  return { type: 'p', id: genEditorId(), children: text ? [{ type: 'text', value: text }] : [] }
}

export function ensureEditableDocument(nodes: EditorNode[]): EditorNode[] {
  return normalizeDocument(nodes.length ? nodes : [emptyParagraph()])
}

export function samePath(a: EditorPath, b: EditorPath): boolean {
  return a.length === b.length && a.every((part, index) => part === b[index])
}

export function normalizeTextRange(range: TextRange): TextRange {
  if (comparePosition(range.anchor, range.focus) <= 0) return range
  return { anchor: range.focus, focus: range.anchor }
}

export function comparePosition(a: TextPosition, b: TextPosition): number {
  const len = Math.min(a.blockPath.length, b.blockPath.length)
  for (let i = 0; i < len; i++) {
    if (a.blockPath[i] !== b.blockPath[i]) return a.blockPath[i] - b.blockPath[i]
  }
  if (a.blockPath.length !== b.blockPath.length) return a.blockPath.length - b.blockPath.length
  return a.offset - b.offset
}

export function replaceTextRange(nodes: EditorNode[], range: TextRange, text: string, marks: InlineMarks = {}): EditorNode[] {
  const ordered = normalizeTextRange(range)
  if (!samePath(ordered.anchor.blockPath, ordered.focus.blockPath)) {
    return insertTextIntoBlock(nodes, ordered.focus.blockPath, ordered.focus.offset, text, marks)
  }

  return updateNodeAtPath(nodes, ordered.anchor.blockPath, (node) => {
    if (!('children' in node) || !Array.isArray(node.children)) return node
    const segments = flattenInlineText(node.children)
    const [beforeStart, afterStart] = splitSegmentsAt(segments, ordered.anchor.offset)
    const [, afterEnd] = splitSegmentsAt(afterStart, ordered.focus.offset - ordered.anchor.offset)
    const inserted = text ? [{ text, marks }] : []
    return { ...node, children: segmentsToInlineNodes([...beforeStart, ...inserted, ...afterEnd]) } as EditorNode
  })
}

export function deleteTextBackward(nodes: EditorNode[], position: TextPosition): { nodes: EditorNode[]; selection: TextPosition } {
  if (position.offset > 0) {
    const next = replaceTextRange(nodes, {
      anchor: { blockPath: position.blockPath, offset: position.offset - 1 },
      focus: position
    }, '')
    return { nodes: next, selection: { blockPath: position.blockPath, offset: position.offset - 1 } }
  }

  return mergeWithPreviousTextBlock(nodes, position.blockPath)
}

export function deleteTextForward(nodes: EditorNode[], position: TextPosition): { nodes: EditorNode[]; selection: TextPosition } {
  const node = getNodeAtPath(nodes, position.blockPath)
  const length = node && 'children' in node && Array.isArray(node.children) ? inlineTextLength(node.children) : 0
  if (position.offset < length) {
    const next = replaceTextRange(nodes, {
      anchor: position,
      focus: { blockPath: position.blockPath, offset: position.offset + 1 }
    }, '')
    return { nodes: next, selection: position }
  }

  return mergeWithNextTextBlock(nodes, position.blockPath)
}

export function mergeWithPreviousTextBlock(nodes: EditorNode[], path: EditorPath): { nodes: EditorNode[]; selection: TextPosition } {
  const previousPath = siblingPath(path, -1)
  if (!previousPath) return { nodes, selection: { blockPath: path, offset: 0 } }
  const current = getNodeAtPath(nodes, path)
  const previous = getNodeAtPath(nodes, previousPath)
  if (!current || !previous) return { nodes, selection: { blockPath: path, offset: 0 } }

  if (isTextBlock(current) && isTextBlock(previous)) {
    const previousLength = inlineTextLength(previous.children)
    const mergedPrevious = { ...previous, children: [...previous.children, ...current.children] } as EditorNode
    const withMerged = updateNodeAtPath(nodes, previousPath, () => mergedPrevious)
    const withoutCurrent = removeNodeAtPath(withMerged, path)
    return { nodes: withoutCurrent, selection: { blockPath: previousPath, offset: previousLength } }
  }

  const shiftedPath = [...path]
  shiftedPath[shiftedPath.length - 1] = Math.max(0, shiftedPath[shiftedPath.length - 1] - 1)
  return { nodes: removeNodeAtPath(nodes, previousPath), selection: { blockPath: shiftedPath, offset: 0 } }
}

export function mergeWithNextTextBlock(nodes: EditorNode[], path: EditorPath): { nodes: EditorNode[]; selection: TextPosition } {
  const nextPath = siblingPath(path, 1)
  const current = getNodeAtPath(nodes, path)
  const next = nextPath ? getNodeAtPath(nodes, nextPath) : null
  const currentLength = current && 'children' in current ? inlineTextLength(current.children) : 0
  if (!nextPath || !current || !next) {
    return { nodes, selection: { blockPath: path, offset: currentLength } }
  }

  if (isTextBlock(current) && isTextBlock(next)) {
    const mergedCurrent = { ...current, children: [...current.children, ...next.children] } as EditorNode
    const withMerged = updateNodeAtPath(nodes, path, () => mergedCurrent)
    const withoutNext = removeNodeAtPath(withMerged, nextPath)
    return { nodes: withoutNext, selection: { blockPath: path, offset: currentLength } }
  }

  return { nodes: removeNodeAtPath(nodes, nextPath), selection: { blockPath: path, offset: currentLength } }
}

function siblingPath(path: EditorPath, delta: -1 | 1): EditorPath | null {
  if (!path.length) return null
  const next = [...path]
  const last = next[next.length - 1] + delta
  if (last < 0) return null
  next[next.length - 1] = last
  return next
}

export function setBlockType(nodes: EditorNode[], path: EditorPath, type: 'p' | 'h1' | 'h2'): EditorNode[] {
  return updateNodeAtPath(nodes, path, (node) => {
    if (!(node.type === 'p' || node.type === 'h1' || node.type === 'h2')) return node
    return { ...node, type, id: node.id || genEditorId(), children: node.children } as EditorNode
  })
}

export function setBlockAlign(nodes: EditorNode[], path: EditorPath, align: 'left' | 'center' | 'right'): EditorNode[] {
  return updateNodeAtPath(nodes, path, (node) => {
    if (!(node.type === 'p' || node.type === 'h1' || node.type === 'h2')) return node
    return { ...node, ...(align === 'left' ? { align: undefined } : { align }) } as EditorNode
  })
}

export function toggleInlineMark(nodes: EditorNode[], range: TextRange, mark: 'strong' | 'em'): EditorNode[] {
  const ordered = normalizeTextRange(range)
  if (!samePath(ordered.anchor.blockPath, ordered.focus.blockPath) || ordered.anchor.offset === ordered.focus.offset) return nodes
  return updateNodeAtPath(nodes, ordered.anchor.blockPath, (node) => {
    if (!('children' in node) || !Array.isArray(node.children)) return node
    const segments = flattenInlineText(node.children)
    const [left, afterStart] = splitSegmentsAt(segments, ordered.anchor.offset)
    const [middle, right] = splitSegmentsAt(afterStart, ordered.focus.offset - ordered.anchor.offset)
    const enabled = !middle.every((seg) => seg.marks[mark])
    const updated = middle.map((seg) => ({ ...seg, marks: { ...seg.marks, [mark]: enabled } }))
    return { ...node, children: segmentsToInlineNodes([...left, ...updated, ...right]) } as EditorNode
  })
}

export function setInlineColor(nodes: EditorNode[], range: TextRange, color: string): EditorNode[] {
  const ordered = normalizeTextRange(range)
  if (!samePath(ordered.anchor.blockPath, ordered.focus.blockPath) || ordered.anchor.offset === ordered.focus.offset) return nodes
  return updateNodeAtPath(nodes, ordered.anchor.blockPath, (node) => {
    if (!('children' in node) || !Array.isArray(node.children)) return node
    const segments = flattenInlineText(node.children)
    const [left, afterStart] = splitSegmentsAt(segments, ordered.anchor.offset)
    const [middle, right] = splitSegmentsAt(afterStart, ordered.focus.offset - ordered.anchor.offset)
    const updated = middle.map((seg) => ({ ...seg, marks: { ...seg.marks, color } }))
    return { ...node, children: segmentsToInlineNodes([...left, ...updated, ...right]) } as EditorNode
  })
}

export function convertTextBlockToTaskList(nodes: EditorNode[], path: EditorPath): EditorNode[] {
  return updateNodeAtPath(nodes, path, (node) => {
    if (!('children' in node) || !Array.isArray(node.children)) return node
    return { type: 'taskList', id: genEditorId(), children: [{ type: 'taskItem', id: genEditorId(), checked: false, children: node.children }] }
  })
}

export function convertTextBlockToUnorderedList(nodes: EditorNode[], path: EditorPath): EditorNode[] {
  return updateNodeAtPath(nodes, path, (node) => {
    if (!('children' in node) || !Array.isArray(node.children)) return node
    return { type: 'ul', id: genEditorId(), children: [{ type: 'li', id: genEditorId(), children: node.children }] }
  })
}