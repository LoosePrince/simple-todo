<script lang="ts">
export type { EditorNode } from '../editor/types'
</script>

<script setup lang="ts">
import { ElMessage } from 'element-plus'
import 'highlight.js/styles/github.css'
import { AlignCenter, AlignLeft, AlignRight, Bold, ChevronDown, ChevronUp, Columns2, Copy, Edit3, Eye, FilePlus, Hand, Image as ImageIcon, Italic, Minus, MousePointer2, PenLine, Plus, Trash2, Type } from 'lucide-vue-next'
import { computed, nextTick, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import '../styles/advanced-editor.css'
import { DEFAULT_CANVAS_HEIGHT, DEFAULT_CANVAS_WIDTH } from '../editor/canvas'
import type { CanvasMode, CanvasObject, MarkdownMode } from '../editor/types'
import {
  deleteNodeById,
  findNodeById,
  findPathById,
  foldDepthAtPath,
  genEditorId,
  insertAfterNodeById,
  isContainerNode,
  isTextBlock,
  normalizeDocument,
  removeAdjacentEmptyParagraphsById,
  updateNodeById
} from '../editor/document'
import { CODE_LANGUAGES, DataEditorNode, attrToPath, findTextContainer, findTextPosition, getFloatingRect, pathToAttr, textOffsetInContainer, type SpecialBlockKind } from '../editor/render'
import { isLikelyCodePaste, normalizePastedText } from '../editor/paste'
import type { EditorNode, InlineMarks } from '../editor/types'
import {
  convertTextBlockToTaskList,
  convertTextBlockToUnorderedList,
  deleteTextBackward,
  deleteTextForward,
  getNodeAtPath,
  inlineTextLength,
  inlineTextValue,
  insertNodesAfterPath,
  insertNodesBeforePath,
  normalizeTextRange,
  replaceNodeAtPathWithNodes,
  replaceTextRange,
  setBlockAlign,
  setBlockType,
  setCodeContent,
  splitTextBlock,
  toggleInlineMark,
  setInlineColor,
  type EditorPath,
  type TextRange,
  type TextPosition
} from '../editor/transactions'

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
  (e: 'upload-canvas-image', payload: { canvasId: string }): void
  (e: 'upload-canvas-file', payload: { canvasId: string }): void
}>()

const { t } = useI18n()
const editorRef = ref<HTMLElement | null>(null)

const selectedImageId = ref<string | null>(null)
const selectedFileId = ref<string | null>(null)
const selectedCodeBlockId = ref<string | null>(null)
const selectedFoldBlockId = ref<string | null>(null)
const selectedMarkdownBlockId = ref<string | null>(null)
const selectedCanvasBlockId = ref<string | null>(null)
const selectedCanvasObjectCanvasId = ref<string | null>(null)
const selectedCanvasObjectId = ref<string | null>(null)
const selectedImageRect = ref({ top: 0, left: 0 })
const selectedFileRect = ref({ top: 0, left: 0 })
const selectedCodeBlockRect = ref({ top: 0, left: 0 })
const selectedFoldBlockRect = ref({ top: 0, left: 0 })
const selectedMarkdownBlockRect = ref({ top: 0, left: 0 })
const selectedCanvasBlockRect = ref({ top: 0, left: 0 })
const selectedCanvasObjectRect = ref({ top: 0, left: 0 })
const currentSelection = ref<TextRange | null>(null)
const lastCursorBlockIndex = ref(0)
const composingPath = ref<string | null>(null)
const composingSelection = ref<TextRange | null>(null)

const undoStack = ref<EditorNode[][]>([])
const redoStack = ref<EditorNode[][]>([])
const MAX_HISTORY = 80
const TOOLBAR_PADDING = 8
const FLOATING_TOOLBAR_WIDTH = 360
const FLOATING_TOOLBAR_HEIGHT = 40

const nodes = computed(() => normalizeDocument(props.modelValue || []))
const selectedImageNode = computed(() => selectedImageId.value ? findNodeById(nodes.value, selectedImageId.value, 'image') as Extract<EditorNode, { type: 'image' }> | null : null)
const selectedFileNode = computed(() => selectedFileId.value ? findNodeById(nodes.value, selectedFileId.value, 'file') as Extract<EditorNode, { type: 'file' }> | null : null)
const selectedCodeBlockNode = computed(() => selectedCodeBlockId.value ? findNodeById(nodes.value, selectedCodeBlockId.value, 'code') as Extract<EditorNode, { type: 'code' }> | null : null)
const selectedFoldBlockNode = computed(() => selectedFoldBlockId.value ? findNodeById(nodes.value, selectedFoldBlockId.value, 'fold') as Extract<EditorNode, { type: 'fold' }> | null : null)
const selectedMarkdownBlockNode = computed(() => selectedMarkdownBlockId.value ? findNodeById(nodes.value, selectedMarkdownBlockId.value, 'markdown') as Extract<EditorNode, { type: 'markdown' }> | null : null)
const selectedCanvasBlockNode = computed(() => selectedCanvasBlockId.value ? findNodeById(nodes.value, selectedCanvasBlockId.value, 'canvas') as Extract<EditorNode, { type: 'canvas' }> | null : null)
const selectedCanvasObjectNode = computed(() => {
  const canvas = selectedCanvasObjectCanvasId.value
    ? findNodeById(nodes.value, selectedCanvasObjectCanvasId.value, 'canvas') as Extract<EditorNode, { type: 'canvas' }> | null
    : null
  return canvas?.objects.find((object) => object.id === selectedCanvasObjectId.value) || null
})
const selectedCanvasTextObject = computed(() => selectedCanvasObjectNode.value?.kind === 'text' ? selectedCanvasObjectNode.value : null)

function updateCanvasObjectFloatingRect(target?: Element | null) {
  const objectId = selectedCanvasObjectId.value
  const element = target || (objectId ? editorRef.value?.querySelector(`[data-canvas-object-id="${objectId}"]`) : null)
  if (!element) return
  selectedCanvasObjectRect.value = getFloatingRect(element, {
    toolbarWidth: FLOATING_TOOLBAR_WIDTH,
    toolbarHeight: FLOATING_TOOLBAR_HEIGHT,
    padding: TOOLBAR_PADDING
  })
}

function pushHistory() {
  const snapshot = structuredClone(nodes.value)
  const last = undoStack.value[undoStack.value.length - 1]
  if (last && JSON.stringify(last) === JSON.stringify(snapshot)) return
  undoStack.value.push(snapshot)
  if (undoStack.value.length > MAX_HISTORY) undoStack.value.shift()
  redoStack.value = []
}

function emitUpdate(next: EditorNode[], selection?: TextRange | TextPosition | null, recordHistory = true) {
  const normalized = normalizeDocument(next)
  if (recordHistory) pushHistory()
  emit('update:modelValue', normalized)
  if (selection) {
    currentSelection.value = 'anchor' in selection ? selection : { anchor: selection, focus: selection }
    nextTick(() => restoreSelection())
  }
}

function applyWithoutHistory(next: EditorNode[], selection?: TextRange | TextPosition | null) {
  emitUpdate(next, selection, false)
}

function readSelectionFromDom(): TextRange | null {
  const sel = window.getSelection()
  if (!sel || sel.rangeCount === 0 || !editorRef.value) return currentSelection.value
  const anchorContainer = findTextContainer(sel.anchorNode)
  const focusContainer = findTextContainer(sel.focusNode)
  if (!anchorContainer || !focusContainer || !editorRef.value.contains(anchorContainer) || !editorRef.value.contains(focusContainer)) {
    return currentSelection.value
  }
  const anchorPath = attrToPath(anchorContainer.getAttribute('data-editor-text-path'))
  const focusPath = attrToPath(focusContainer.getAttribute('data-editor-text-path'))
  if (!anchorPath || !focusPath) return currentSelection.value
  return {
    anchor: { blockPath: anchorPath, offset: textOffsetInContainer(anchorContainer, sel.anchorNode!, sel.anchorOffset) },
    focus: { blockPath: focusPath, offset: textOffsetInContainer(focusContainer, sel.focusNode!, sel.focusOffset) }
  }
}

function captureSelection() {
  const range = readSelectionFromDom()
  if (range) {
    currentSelection.value = range
    lastCursorBlockIndex.value = range.focus.blockPath[0] ?? 0
  }
}

function restoreSelection(selection = currentSelection.value) {
  if (!selection || !editorRef.value) return
  const point = selection.focus
  const container = editorRef.value.querySelector<HTMLElement>(`[data-editor-text-path="${pathToAttr(point.blockPath)}"]`)
  if (!container) return
  const pos = findTextPosition(container, point.offset)
  const range = document.createRange()
  range.setStart(pos.node, pos.offset)
  range.collapse(true)
  const sel = window.getSelection()
  if (!sel) return
  sel.removeAllRanges()
  sel.addRange(range)
}

const saveSelection = () => captureSelection()
const restoreSavedSelection = () => restoreSelection()

function marksAtSelection(): InlineMarks {
  const range = normalizeTextRange(currentSelection.value || { anchor: { blockPath: [0], offset: 0 }, focus: { blockPath: [0], offset: 0 } })
  const node = getNodeAtPath(nodes.value, range.focus.blockPath)
  if (!node || !('children' in node) || !Array.isArray(node.children)) return {}
  return {}
}

function updateFloatingRect(target: HTMLElement, kind: SpecialBlockKind) {
  const next = getFloatingRect(target, {
    toolbarWidth: FLOATING_TOOLBAR_WIDTH,
    toolbarHeight: FLOATING_TOOLBAR_HEIGHT,
    padding: TOOLBAR_PADDING
  })
  if (kind === 'image') selectedImageRect.value = next
  if (kind === 'file') selectedFileRect.value = next
  if (kind === 'code') selectedCodeBlockRect.value = next
  if (kind === 'fold') selectedFoldBlockRect.value = next
  if (kind === 'markdown') selectedMarkdownBlockRect.value = next
  if (kind === 'canvas') selectedCanvasBlockRect.value = next
}

function clearFloatingSelection(except?: SpecialBlockKind | 'canvas-object') {
  if (except !== 'image') selectedImageId.value = null
  if (except !== 'file') selectedFileId.value = null
  if (except !== 'code') selectedCodeBlockId.value = null
  if (except !== 'fold') selectedFoldBlockId.value = null
  if (except !== 'markdown') selectedMarkdownBlockId.value = null
  if (except !== 'canvas') selectedCanvasBlockId.value = null
  if (except !== 'canvas-object') {
    selectedCanvasObjectCanvasId.value = null
    selectedCanvasObjectId.value = null
  }
}

function selectSpecialBlock(id: string, kind: SpecialBlockKind, target: HTMLElement) {
  clearFloatingSelection(kind)
  if (kind === 'image') selectedImageId.value = id
  if (kind === 'file') selectedFileId.value = id
  if (kind === 'code') selectedCodeBlockId.value = id
  if (kind === 'fold') selectedFoldBlockId.value = id
  if (kind === 'markdown') selectedMarkdownBlockId.value = id
  if (kind === 'canvas') selectedCanvasBlockId.value = id
  updateFloatingRect(target, kind)
}

function handleTextFocus(path: EditorPath, event: FocusEvent) {
  const text = inlineTextLength((getNodeAtPath(nodes.value, path) as any)?.children || [])
  currentSelection.value = { anchor: { blockPath: path, offset: text }, focus: { blockPath: path, offset: text } }
  lastCursorBlockIndex.value = path[0] ?? 0
  clearFloatingSelection()
  nextTick(() => captureSelection())
}

function handleTextSelect() {
  captureSelection()
}

function replaceSelectionWithText(text: string, marks: InlineMarks = marksAtSelection()) {
  const range = currentSelection.value || readSelectionFromDom()
  if (!range) return
  const ordered = normalizeTextRange(range)
  const next = replaceTextRange(nodes.value, ordered, text, marks)
  const selection = { blockPath: ordered.anchor.blockPath, offset: ordered.anchor.offset + text.length }
  emitUpdate(next, selection)
}

function isCollapsedRange(range: TextRange): boolean {
  return pathToAttr(range.anchor.blockPath) === pathToAttr(range.focus.blockPath) && range.anchor.offset === range.focus.offset
}

function deleteSelectionOrAdjacent(direction: 'backward' | 'forward') {
  captureSelection()
  const range = currentSelection.value
  if (!range) return
  const ordered = normalizeTextRange(range)
  if (!isCollapsedRange(ordered)) {
    const next = replaceTextRange(nodes.value, ordered, '')
    emitUpdate(next, ordered.anchor)
    return
  }

  const result = direction === 'backward'
    ? deleteTextBackward(nodes.value, ordered.focus)
    : deleteTextForward(nodes.value, ordered.focus)
  emitUpdate(result.nodes, result.selection)
}

interface TextBlockEntry {
  path: EditorPath
  length: number
}

function sameEditorPath(a: EditorPath, b: EditorPath): boolean {
  return a.length === b.length && a.every((part, index) => part === b[index])
}

function collectTextBlockEntries(items: EditorNode[] = nodes.value, basePath: EditorPath = []): TextBlockEntry[] {
  const entries: TextBlockEntry[] = []
  items.forEach((node, index) => {
    const path = [...basePath, index]
    if (isTextBlock(node)) {
      entries.push({ path, length: inlineTextLength(node.children) })
      return
    }
    if (node.type === 'fold' && node.folded) return
    if (isContainerNode(node)) entries.push(...collectTextBlockEntries(node.children, path))
  })
  return entries
}

function adjacentDocumentTextPosition(path: EditorPath, direction: 'previous' | 'next', mode: 'start' | 'end' | 'preserve', sourceOffset = 0): TextPosition | null {
  const entries = collectTextBlockEntries()
  const index = entries.findIndex((entry) => sameEditorPath(entry.path, path))
  if (index < 0) return null
  const target = entries[index + (direction === 'previous' ? -1 : 1)]
  if (!target) return null
  const offset = mode === 'start' ? 0 : mode === 'end' ? target.length : Math.min(sourceOffset, target.length)
  return { blockPath: target.path, offset }
}

function caretRectForPosition(path: EditorPath, offset: number): DOMRect | null {
  const container = editorRef.value?.querySelector<HTMLElement>(`[data-editor-text-path="${pathToAttr(path)}"]`)
  if (!container) return null
  const position = findTextPosition(container, offset)
  const range = document.createRange()
  range.setStart(position.node, position.offset)
  range.collapse(true)
  const rect = range.getClientRects()[0]
  return rect || container.getBoundingClientRect()
}

function isCaretOnVisualBoundary(path: EditorPath, offset: number, side: 'first' | 'last'): boolean {
  const container = editorRef.value?.querySelector<HTMLElement>(`[data-editor-text-path="${pathToAttr(path)}"]`)
  const caretRect = caretRectForPosition(path, offset)
  if (!container || !caretRect) return false
  const containerRect = container.getBoundingClientRect()
  const lineHeight = Number.parseFloat(getComputedStyle(container).lineHeight) || 20
  return side === 'first'
    ? caretRect.top <= containerRect.top + lineHeight / 2
    : caretRect.bottom >= containerRect.bottom - lineHeight / 2
}

function moveToAdjacentTextBlock(direction: 'previous' | 'next', mode: 'start' | 'end' | 'preserve', sourceOffset = 0): boolean {
  const range = currentSelection.value
  if (!range) return false
  const ordered = normalizeTextRange(range)
  if (!isCollapsedRange(ordered)) return false
  const position = adjacentDocumentTextPosition(ordered.focus.blockPath, direction, mode, sourceOffset)
  if (!position) return false
  clearFloatingSelection()
  setCollapsedSelection(position)
  return true
}

function handleArrowNavigation(event: KeyboardEvent, path: EditorPath): boolean {
  if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return false
  if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return false
  captureSelection()
  const range = currentSelection.value
  if (!range) return false
  const ordered = normalizeTextRange(range)
  if (!isCollapsedRange(ordered) || !sameEditorPath(ordered.focus.blockPath, path)) return false
  const node = getNodeAtPath(nodes.value, path)
  const length = node && isTextBlock(node) ? inlineTextLength(node.children) : 0

  if (event.key === 'ArrowLeft' && ordered.focus.offset === 0) {
    event.preventDefault()
    return moveToAdjacentTextBlock('previous', 'end')
  }
  if (event.key === 'ArrowRight' && ordered.focus.offset === length) {
    event.preventDefault()
    return moveToAdjacentTextBlock('next', 'start')
  }
  if (event.key === 'ArrowUp' && isCaretOnVisualBoundary(path, ordered.focus.offset, 'first')) {
    event.preventDefault()
    return moveToAdjacentTextBlock('previous', 'preserve', ordered.focus.offset)
  }
  if (event.key === 'ArrowDown' && isCaretOnVisualBoundary(path, ordered.focus.offset, 'last')) {
    event.preventDefault()
    return moveToAdjacentTextBlock('next', 'preserve', ordered.focus.offset)
  }
  return false
}

function handleBeforeInput(payload: { event: InputEvent; path: EditorPath }) {
  const e = payload.event
  if (composingPath.value) return
  captureSelection()
  const range = currentSelection.value
  if (!range) return

  if (e.inputType === 'insertText') {
    e.preventDefault()
    replaceSelectionWithText(e.data || '')
    return
  }

  if (e.inputType === 'insertLineBreak') {
    e.preventDefault()
    replaceSelectionWithText('\n')
    return
  }

  if (e.inputType === 'insertParagraph') {
    e.preventDefault()
    const ordered = normalizeTextRange(range)
    if (ordered.anchor.offset !== ordered.focus.offset) {
      const withoutSelection = replaceTextRange(nodes.value, ordered, '')
      const split = splitTextBlock(withoutSelection, ordered.anchor.blockPath, ordered.anchor.offset)
      emitUpdate(split, { blockPath: [...ordered.anchor.blockPath.slice(0, -1), ordered.anchor.blockPath[ordered.anchor.blockPath.length - 1] + 1], offset: 0 })
    } else {
      const split = splitTextBlock(nodes.value, ordered.focus.blockPath, ordered.focus.offset)
      emitUpdate(split, { blockPath: [...ordered.focus.blockPath.slice(0, -1), ordered.focus.blockPath[ordered.focus.blockPath.length - 1] + 1], offset: 0 })
    }
    return
  }

  if (e.inputType === 'deleteContentBackward') {
    e.preventDefault()
    deleteSelectionOrAdjacent('backward')
    return
  }

  if (e.inputType === 'deleteContentForward') {
    e.preventDefault()
    deleteSelectionOrAdjacent('forward')
    return
  }
}

function handleTextKeydown(payload: { event: KeyboardEvent; path: EditorPath }) {
  const e = payload.event
  if (e.key === 'Backspace') {
    e.preventDefault()
    deleteSelectionOrAdjacent('backward')
    return
  }
  if (e.key === 'Delete') {
    e.preventDefault()
    deleteSelectionOrAdjacent('forward')
    return
  }
  if (handleArrowNavigation(e, payload.path)) return
  if (e.key === 'Enter') {
    e.preventDefault()
    captureSelection()
    const range = normalizeTextRange(currentSelection.value || { anchor: { blockPath: payload.path, offset: 0 }, focus: { blockPath: payload.path, offset: 0 } })
    if (e.shiftKey) {
      replaceSelectionWithText('\n')
      return
    }
    const split = splitTextBlock(nodes.value, range.focus.blockPath, range.focus.offset)
    emitUpdate(split, { blockPath: [...range.focus.blockPath.slice(0, -1), range.focus.blockPath[range.focus.blockPath.length - 1] + 1], offset: 0 })
  }
}

function handlePaste(payload: { event: ClipboardEvent; path: EditorPath }) {
  const e = payload.event
  const data = e.clipboardData
  if (!data) return
  const files = Array.from(data.items || [])
    .filter((item) => item.kind === 'file')
    .map((item) => item.getAsFile())
    .filter((file): file is File => !!file)
  const text = data.getData('text/plain') ?? ''
  e.preventDefault()
  captureSelection()
  if (files.length) {
    emit('paste-files', { files, text })
    return
  }
  if (text) {
    const normalized = normalizePastedText(text)
    if (isLikelyCodePaste(normalized)) {
      emit('paste-files', { files: [], text: normalized })
      return
    }
    replaceSelectionWithText(normalized)
  }
}

function handleCompositionStart(path: EditorPath) {
  captureSelection()
  composingPath.value = pathToAttr(path)
  composingSelection.value = currentSelection.value ? structuredClone(currentSelection.value) : null
}

function handleCompositionEnd(payload: { event: CompositionEvent; path: EditorPath }) {
  const text = payload.event.data || ''
  composingPath.value = null
  if (composingSelection.value) currentSelection.value = composingSelection.value
  composingSelection.value = null
  if (text) replaceSelectionWithText(text)
}

function handleCodeInput(id: string, content: string) {
  emitUpdate(setCodeContent(nodes.value, id, content), null)
}

function execCommand(command: string, value?: string) {
  captureSelection()
  const selection = currentSelection.value
  if (command === 'undo') {
    const prev = undoStack.value.pop()
    if (!prev) return
    redoStack.value.push(structuredClone(nodes.value))
    applyWithoutHistory(prev, selection)
    return
  }
  if (command === 'redo') {
    const next = redoStack.value.pop()
    if (!next) return
    undoStack.value.push(structuredClone(nodes.value))
    applyWithoutHistory(next, selection)
    return
  }
  if (!selection) return
  const ordered = normalizeTextRange(selection)
  let next = nodes.value
  if (command === 'bold') next = toggleInlineMark(next, ordered, 'strong')
  else if (command === 'italic') next = toggleInlineMark(next, ordered, 'em')
  else if (command === 'foreColor' && value) next = setInlineColor(next, ordered, value)
  else if (command === 'formatBlock' && (value === 'p' || value === 'h1' || value === 'h2')) next = setBlockType(next, ordered.focus.blockPath, value)
  else if (command === 'justifyLeft') next = setBlockAlign(next, ordered.focus.blockPath, 'left')
  else if (command === 'justifyCenter') next = setBlockAlign(next, ordered.focus.blockPath, 'center')
  else if (command === 'justifyRight') next = setBlockAlign(next, ordered.focus.blockPath, 'right')
  else if (command === 'insertUnorderedList') next = convertTextBlockToUnorderedList(next, ordered.focus.blockPath)
  emitUpdate(next, ordered)
}

function insertPlainText(text: string) {
  captureSelection()
  replaceSelectionWithText(text.replace(/\r\n/g, '\n'))
}

function insertNodesAtSelection(newNodes: EditorNode | EditorNode[]) {
  captureSelection()
  const insertNodes = Array.isArray(newNodes) ? newNodes : [newNodes]
  const selection = currentSelection.value
  if (!selection) {
    emitUpdate([...nodes.value, ...insertNodes], null)
    return
  }
  const path = normalizeTextRange(selection).focus.blockPath
  const target = getNodeAtPath(nodes.value, path)
  const emptyText = target && 'children' in target && inlineTextValue(target.children).length === 0
  const next = emptyText ? replaceNodeAtPathWithNodes(nodes.value, path, insertNodes) : insertNodesAfterPath(nodes.value, path, insertNodes)
  emitUpdate(next, null)
}

function insertTaskListAtSelection() {
  captureSelection()
  const path = normalizeTextRange(currentSelection.value || { anchor: { blockPath: [0], offset: 0 }, focus: { blockPath: [0], offset: 0 } }).focus.blockPath
  emitUpdate(convertTextBlockToTaskList(nodes.value, path), null)
}

function insertCodeBlock() {
  insertNodesAtSelection({ type: 'code', id: genEditorId(), content: '', language: 'text' })
}

function insertMarkdownBlock() {
  insertNodesAtSelection({ type: 'markdown', id: genEditorId(), content: '', mode: 'edit' })
}

function insertCanvasBlock() {
  insertNodesAtSelection({
    type: 'canvas',
    id: genEditorId(),
    width: DEFAULT_CANVAS_WIDTH,
    height: DEFAULT_CANVAS_HEIGHT,
    mode: 'select',
    objects: []
  })
}

function insertFoldBlock() {
  captureSelection()
  const path = normalizeTextRange(currentSelection.value || { anchor: { blockPath: [0], offset: 0 }, focus: { blockPath: [0], offset: 0 } }).focus.blockPath
  if (foldDepthAtPath(nodes.value, path) >= 3) return
  insertNodesAtSelection({ type: 'fold', id: genEditorId(), folded: false, children: [{ type: 'p', id: genEditorId(), children: [] }] })
}

function updateImageNode(updates: Partial<Extract<EditorNode, { type: 'image' }>>) {
  if (!selectedImageId.value) return
  emitUpdate(updateNodeById(nodes.value, selectedImageId.value, updates as Partial<EditorNode>), null)
}

function updateFileNode(updates: Partial<Extract<EditorNode, { type: 'file' }>>) {
  if (!selectedFileId.value) return
  emitUpdate(updateNodeById(nodes.value, selectedFileId.value, updates as Partial<EditorNode>), null)
}

function handleMarkdownInput(id: string, content: string) {
  emitUpdate(updateNodeById(nodes.value, id, { content } as Partial<EditorNode>), null)
}

function setMarkdownMode(id: string, mode: MarkdownMode) {
  emitUpdate(updateNodeById(nodes.value, id, { mode } as Partial<EditorNode>), null)
}

function handleCanvasObjectsChange(id: string, objects: CanvasObject[]) {
  emitUpdate(updateNodeById(nodes.value, id, { objects } as Partial<EditorNode>), null)
  if (id === selectedCanvasObjectCanvasId.value) nextTick(() => updateCanvasObjectFloatingRect())
}

function updateSelectedCanvasObject(updates: Partial<CanvasObject>) {
  const canvasId = selectedCanvasObjectCanvasId.value
  const objectId = selectedCanvasObjectId.value
  const canvas = canvasId ? findNodeById(nodes.value, canvasId, 'canvas') as Extract<EditorNode, { type: 'canvas' }> | null : null
  if (!canvasId || !canvas || !objectId) return
  const objects = canvas.objects.map((object) => object.id === objectId ? { ...object, ...updates } as CanvasObject : object)
  emitUpdate(updateNodeById(nodes.value, canvasId, { objects } as Partial<EditorNode>), null)
  nextTick(() => updateCanvasObjectFloatingRect())
}

function deleteSelectedCanvasObject() {
  const canvasId = selectedCanvasObjectCanvasId.value
  const objectId = selectedCanvasObjectId.value
  const canvas = canvasId ? findNodeById(nodes.value, canvasId, 'canvas') as Extract<EditorNode, { type: 'canvas' }> | null : null
  if (!canvasId || !canvas || !objectId) return
  const objects = canvas.objects.filter((object) => object.id !== objectId)
  emitUpdate(updateNodeById(nodes.value, canvasId, { objects } as Partial<EditorNode>), null)
  selectedCanvasObjectCanvasId.value = null
  selectedCanvasObjectId.value = null
}

function toggleSelectedCanvasTextBold() {
  const object = selectedCanvasTextObject.value
  if (!object) return
  updateSelectedCanvasObject({ fontWeight: object.fontWeight === 'bold' ? 'normal' : 'bold' } as Partial<CanvasObject>)
}

function toggleSelectedCanvasTextItalic() {
  const object = selectedCanvasTextObject.value
  if (!object) return
  updateSelectedCanvasObject({ fontStyle: object.fontStyle === 'italic' ? 'normal' : 'italic' } as Partial<CanvasObject>)
}

function setSelectedCanvasTextColor(color: string) {
  if (!selectedCanvasTextObject.value || !color) return
  updateSelectedCanvasObject({ color } as Partial<CanvasObject>)
}

function handleSelectedCanvasTextColorChange(value: string | null) {
  setSelectedCanvasTextColor(String(value || ''))
}

function setSelectedCanvasTextAlign(textAlign: 'left' | 'center' | 'right') {
  if (!selectedCanvasTextObject.value) return
  updateSelectedCanvasObject({ textAlign } as Partial<CanvasObject>)
}

function setCanvasMode(id: string, mode: CanvasMode) {
  emitUpdate(updateNodeById(nodes.value, id, { mode } as Partial<EditorNode>), null)
}

function appendCanvasObject(canvasId: string, object: CanvasObject) {
  const node = findNodeById(nodes.value, canvasId, 'canvas') as Extract<EditorNode, { type: 'canvas' }> | null
  if (!node) return
  emitUpdate(updateNodeById(nodes.value, canvasId, { objects: [...node.objects, object] } as Partial<EditorNode>), null)
}

function insertCanvasTextObject(canvasId = selectedCanvasBlockId.value) {
  if (!canvasId) return
  appendCanvasObject(canvasId, {
    id: genEditorId(),
    kind: 'text',
    text: 'Text',
    x: 80,
    y: 80,
    width: 180,
    height: 72,
    rotation: 0,
    fontSize: 20,
    color: '#111827'
  })
}

function insertCanvasImageObject(canvasId: string, asset: { url: string; assetPath: string; width?: number; height?: number }) {
  appendCanvasObject(canvasId, {
    id: genEditorId(),
    kind: 'image',
    url: asset.url,
    assetPath: asset.assetPath,
    x: 80,
    y: 80,
    width: asset.width || 220,
    height: asset.height || 160,
    rotation: 0
  })
}

function insertCanvasFileObject(canvasId: string, asset: { url: string; assetPath: string; fileName: string; fileSize?: number }) {
  appendCanvasObject(canvasId, {
    id: genEditorId(),
    kind: 'file',
    url: asset.url,
    assetPath: asset.assetPath,
    fileName: asset.fileName,
    fileSize: asset.fileSize,
    x: 80,
    y: 80,
    width: 240,
    height: 84,
    rotation: 0
  })
}

function setCodeBlockLanguage(id: string, language: string) {
  emitUpdate(updateNodeById(nodes.value, id, { language } as Partial<EditorNode>), null)
}

async function copyCodeBlockContent(id: string) {
  const node = findNodeById(nodes.value, id, 'code') as Extract<EditorNode, { type: 'code' }> | null
  if (!node) return
  await navigator.clipboard.writeText(node.content || '')
  ElMessage.success(t('editor.copySuccess'))
}

function deleteBlockById(id: string) {
  emitUpdate(deleteNodeById(nodes.value, id), null)
  clearFloatingSelection()
}

function toggleFoldBlock(id: string) {
  const node = findNodeById(nodes.value, id, 'fold') as Extract<EditorNode, { type: 'fold' }> | null
  if (!node) return
  emitUpdate(updateNodeById(nodes.value, id, { folded: !node.folded } as Partial<EditorNode>), null)
}

function insertAfterSelectedCodeBlock() {
  if (!selectedCodeBlockId.value) return
  emitUpdate(insertAfterNodeById(nodes.value, selectedCodeBlockId.value, { type: 'p', id: genEditorId(), children: [] }), null)
}

function insertAfterSelectedFoldBlock() {
  if (!selectedFoldBlockId.value) return
  emitUpdate(insertAfterNodeById(nodes.value, selectedFoldBlockId.value, { type: 'p', id: genEditorId(), children: [] }), null)
}

function insertAfterSelectedMarkdownBlock() {
  if (!selectedMarkdownBlockId.value) return
  emitUpdate(insertAfterNodeById(nodes.value, selectedMarkdownBlockId.value, { type: 'p', id: genEditorId(), children: [] }), null)
}

function insertAfterSelectedCanvasBlock() {
  if (!selectedCanvasBlockId.value) return
  emitUpdate(insertAfterNodeById(nodes.value, selectedCanvasBlockId.value, { type: 'p', id: genEditorId(), children: [] }), null)
}

function removeEmptyLinesAroundSelectedCodeBlock() {
  if (selectedCodeBlockId.value) removeAdjacentEmptyParagraphs(selectedCodeBlockId.value)
}

function removeEmptyLinesAroundSelectedFoldBlock() {
  if (selectedFoldBlockId.value) removeAdjacentEmptyParagraphs(selectedFoldBlockId.value)
}

function removeEmptyLinesAroundSelectedMarkdownBlock() {
  if (selectedMarkdownBlockId.value) removeAdjacentEmptyParagraphs(selectedMarkdownBlockId.value)
}

function removeEmptyLinesAroundSelectedCanvasBlock() {
  if (selectedCanvasBlockId.value) removeAdjacentEmptyParagraphs(selectedCanvasBlockId.value)
}

function removeAdjacentEmptyParagraphs(id: string) {
  const next = removeAdjacentEmptyParagraphsById(nodes.value, id)
  emitUpdate(next, null)
}

function handleTaskToggle(id: string, checked: boolean) {
  emitUpdate(updateNodeById(nodes.value, id, { checked } as Partial<EditorNode>), null)
}

function adjacentTextPosition(path: EditorPath, side: 'before' | 'after'): TextPosition | null {
  if (!path.length) return null
  const siblingPath = [...path]
  const lastIndex = siblingPath.length - 1
  siblingPath[lastIndex] += side === 'before' ? -1 : 1
  if (siblingPath[lastIndex] < 0) return null
  const sibling = getNodeAtPath(nodes.value, siblingPath)
  if (!sibling || !isTextBlock(sibling)) return null
  return {
    blockPath: siblingPath,
    offset: side === 'before' ? inlineTextLength(sibling.children) : 0
  }
}

function textPositionAtEndOfNode(node: EditorNode, path: EditorPath): TextPosition | null {
  if (isTextBlock(node)) return { blockPath: path, offset: inlineTextLength(node.children) }
  if (node.type === 'ul' || node.type === 'ol' || node.type === 'taskList') {
    for (let index = node.children.length - 1; index >= 0; index -= 1) {
      const position = textPositionAtEndOfNode(node.children[index], [...path, index])
      if (position) return position
    }
  }
  return null
}

function setCollapsedSelection(position: TextPosition) {
  currentSelection.value = { anchor: position, focus: position }
  lastCursorBlockIndex.value = position.blockPath[0] ?? 0
  nextTick(() => restoreSelection())
}

function insertParagraphAroundPath(path: EditorPath, side: 'before' | 'after') {
  const paragraph: EditorNode = { type: 'p', id: genEditorId(), children: [] }
  const insertedPath = side === 'before'
    ? path
    : [...path.slice(0, -1), path[path.length - 1] + 1]
  const next = side === 'before'
    ? insertNodesBeforePath(nodes.value, path, [paragraph])
    : insertNodesAfterPath(nodes.value, path, [paragraph])
  emitUpdate(next, { blockPath: insertedPath, offset: 0 })
}

function placeCursorAtDocumentEnd() {
  clearFloatingSelection()
  const lastIndex = nodes.value.length - 1
  const lastNode = nodes.value[lastIndex]
  if (!lastNode) {
    emitUpdate([{ type: 'p', id: genEditorId(), children: [] }], { blockPath: [0], offset: 0 })
    return
  }

  const textPosition = textPositionAtEndOfNode(lastNode, [lastIndex])
  if (textPosition) {
    setCollapsedSelection(textPosition)
    return
  }

  insertParagraphAroundPath([lastIndex], 'after')
}

function handleEditorClick(event: MouseEvent) {
  if (event.target === editorRef.value) {
    event.preventDefault()
    placeCursorAtDocumentEnd()
    return
  }
  captureSelection()
}

function handleBlockBoundaryClick(payload: { path: EditorPath; side: 'before' | 'after'; event: MouseEvent }) {
  clearFloatingSelection()
  const adjacentPosition = adjacentTextPosition(payload.path, payload.side)
  if (adjacentPosition) {
    setCollapsedSelection(adjacentPosition)
    return
  }

  insertParagraphAroundPath(payload.path, payload.side)
}

function handleNodeClick(payload: { id: string; type: SpecialBlockKind; event: MouseEvent | FocusEvent }) {
  const target = payload.event.currentTarget as HTMLElement
  const path = findPathById(nodes.value, payload.id)
  if (path) lastCursorBlockIndex.value = path[0] ?? 0
  selectSpecialBlock(payload.id, payload.type, target)
}

function handleCanvasObjectSelect(payload: { canvasId: string; objectId: string | null; object?: CanvasObject; event: PointerEvent | MouseEvent | FocusEvent }) {
  if (!payload.objectId) {
    selectedCanvasObjectCanvasId.value = null
    selectedCanvasObjectId.value = null
    return
  }
  const path = findPathById(nodes.value, payload.canvasId)
  if (path) lastCursorBlockIndex.value = path[0] ?? 0
  clearFloatingSelection('canvas-object')
  selectedCanvasObjectCanvasId.value = payload.canvasId
  selectedCanvasObjectId.value = payload.objectId
  const target = payload.event.currentTarget as Element | null
  nextTick(() => updateCanvasObjectFloatingRect(target?.closest?.('[data-canvas-object-id]') || target))
}

function isEditableKeyboardTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  return !!target.closest('textarea,input,[contenteditable="true"]')
}

function handleEditorKeydown(event: KeyboardEvent) {
  if ((event.key === 'Delete' || event.key === 'Backspace') && selectedCanvasObjectId.value && !isEditableKeyboardTarget(event.target)) {
    event.preventDefault()
    deleteSelectedCanvasObject()
  }
}

function handleNodeContextmenu(payload: { id: string; type: 'image' | 'file'; assetPath?: string; event: MouseEvent }) {
  if (!payload.assetPath) return
  payload.event.preventDefault()
  emit('contextmenu', { type: payload.type, assetPath: payload.assetPath, id: payload.id, clientX: payload.event.clientX, clientY: payload.event.clientY })
}

function handleOpenAsset(payload: { id: string; type: 'image' | 'file'; assetPath?: string }) {
  if (!payload.assetPath) return
  emit('open-asset', { type: payload.type, assetPath: payload.assetPath, id: payload.id })
}

defineExpose({
  execCommand,
  handleInput: () => emit('update:modelValue', normalizeDocument(nodes.value)),
  saveSelection,
  restoreSelection: restoreSavedSelection,
  getCursorBlockIndex: () => lastCursorBlockIndex.value,
  insertTaskListAtSelection,
  insertPlainText,
  insertCodeBlock,
  insertMarkdownBlock,
  insertCanvasBlock,
  insertFoldBlock,
  insertCanvasTextObject,
  insertCanvasImageObject,
  insertCanvasFileObject,
  insertNodesAtSelection
})
</script>

<template>
  <div class="advanced-editor-container">
    <div ref="editorRef" class="advanced-editor" @click="handleEditorClick" @keydown="handleEditorKeydown">
      <DataEditorNode
        v-for="(node, index) in nodes"
        :key="'id' in node ? node.id : index"
        :node="node"
        :path="[index]"
        :depth="0"
        :selected-canvas-object-id="selectedCanvasObjectId"
        @text-focus="(payload) => handleTextFocus(payload.path, payload.event)"
        @text-select="handleTextSelect"
        @text-beforeinput="handleBeforeInput"
        @text-keydown="handleTextKeydown"
        @text-paste="handlePaste"
        @composition-start="handleCompositionStart"
        @composition-end="handleCompositionEnd"
        @node-click="handleNodeClick"
        @node-contextmenu="handleNodeContextmenu"
        @open-asset="handleOpenAsset"
        @task-toggle="(payload) => handleTaskToggle(payload.id, payload.checked)"
        @code-input="(payload) => handleCodeInput(payload.id, payload.content)"
        @markdown-input="(payload) => handleMarkdownInput(payload.id, payload.content)"
        @canvas-objects-change="(payload) => handleCanvasObjectsChange(payload.id, payload.objects)"
        @canvas-object-select="handleCanvasObjectSelect"
        @block-boundary-click="handleBlockBoundaryClick"
        @fold-toggle="(payload) => toggleFoldBlock(payload.id)"
      />
    </div>

    <Teleport to="body">
      <div v-if="selectedImageId && selectedImageNode" class="image-toolbar-root image-toolbar"
        :style="{ top: selectedImageRect.top + 'px', left: selectedImageRect.left + 'px' }">
        <label class="image-toolbar-label">{{ t('editor.imageWidthPercent') }}</label>
        <input type="number" class="image-toolbar-input" min="10" max="100"
          :value="selectedImageNode.widthPercent ?? 100"
          @change="(e) => updateImageNode({ widthPercent: Math.min(100, Math.max(10, Number((e.target as HTMLInputElement).value) || 100)) })" />
        <el-tooltip :content="t('editor.toolbarAlignLeft')" placement="top">
          <button type="button" class="image-toolbar-btn" @click="updateImageNode({ align: 'left' })"><AlignLeft :size="16" /></button>
        </el-tooltip>
        <el-tooltip :content="t('editor.toolbarAlignCenter')" placement="top">
          <button type="button" class="image-toolbar-btn" @click="updateImageNode({ align: 'center' })"><AlignCenter :size="16" /></button>
        </el-tooltip>
        <el-tooltip :content="t('editor.toolbarAlignRight')" placement="top">
          <button type="button" class="image-toolbar-btn" @click="updateImageNode({ align: 'right' })"><AlignRight :size="16" /></button>
        </el-tooltip>
      </div>

      <div v-if="selectedCodeBlockId && selectedCodeBlockNode" class="code-toolbar-root code-toolbar"
        :style="{ top: selectedCodeBlockRect.top + 'px', left: selectedCodeBlockRect.left + 'px' }">
        <select class="code-toolbar-select" :value="selectedCodeBlockNode.language || 'text'"
          @change="(e) => setCodeBlockLanguage(selectedCodeBlockId!, (e.target as HTMLSelectElement).value)">
          <option v-for="opt in CODE_LANGUAGES" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
        </select>
        <el-tooltip :content="t('editor.codeCopy')" placement="top">
          <button type="button" class="code-toolbar-btn" @click="copyCodeBlockContent(selectedCodeBlockId!)"><Copy :size="16" /></button>
        </el-tooltip>
        <el-tooltip :content="t('editor.foldInsertAfter')" placement="top">
          <button type="button" class="code-toolbar-btn" @click="insertAfterSelectedCodeBlock"><Plus :size="16" /></button>
        </el-tooltip>
        <el-tooltip :content="t('editor.removeEmptyLines')" placement="top">
          <button type="button" class="code-toolbar-btn" @click="removeEmptyLinesAroundSelectedCodeBlock"><Minus :size="16" /></button>
        </el-tooltip>
        <el-tooltip :content="t('editor.deleteBlock')" placement="top">
          <button type="button" class="code-toolbar-btn" @click="deleteBlockById(selectedCodeBlockId!)"><Trash2 :size="16" /></button>
        </el-tooltip>
      </div>

      <div v-if="selectedFoldBlockId && selectedFoldBlockNode" class="fold-toolbar-root fold-toolbar"
        :style="{ top: selectedFoldBlockRect.top + 'px', left: selectedFoldBlockRect.left + 'px' }">
        <el-tooltip :content="selectedFoldBlockNode.folded ? t('editor.expandFold') : t('editor.collapseFold')" placement="top">
          <button type="button" class="fold-toolbar-btn" @click="toggleFoldBlock(selectedFoldBlockId!)">
            <ChevronDown v-if="selectedFoldBlockNode.folded" :size="16" />
            <ChevronUp v-else :size="16" />
          </button>
        </el-tooltip>
        <el-tooltip :content="t('editor.foldInsertAfter')" placement="top">
          <button type="button" class="fold-toolbar-btn" @click="insertAfterSelectedFoldBlock"><Plus :size="16" /></button>
        </el-tooltip>
        <el-tooltip :content="t('editor.removeEmptyLines')" placement="top">
          <button type="button" class="fold-toolbar-btn" @click="removeEmptyLinesAroundSelectedFoldBlock"><Minus :size="16" /></button>
        </el-tooltip>
        <el-tooltip :content="t('editor.deleteBlock')" placement="top">
          <button type="button" class="fold-toolbar-btn" @click="deleteBlockById(selectedFoldBlockId!)"><Trash2 :size="16" /></button>
        </el-tooltip>
      </div>

      <div v-if="selectedMarkdownBlockId && selectedMarkdownBlockNode" class="markdown-toolbar-root markdown-toolbar"
        :style="{ top: selectedMarkdownBlockRect.top + 'px', left: selectedMarkdownBlockRect.left + 'px' }">
        <el-tooltip :content="t('editor.markdownEdit')" placement="top">
          <button type="button" class="markdown-toolbar-btn" :class="{ 'is-active': selectedMarkdownBlockNode.mode === 'edit' }" @click="setMarkdownMode(selectedMarkdownBlockId!, 'edit')"><Edit3 :size="16" /></button>
        </el-tooltip>
        <el-tooltip :content="t('editor.markdownPreview')" placement="top">
          <button type="button" class="markdown-toolbar-btn" :class="{ 'is-active': selectedMarkdownBlockNode.mode === 'preview' }" @click="setMarkdownMode(selectedMarkdownBlockId!, 'preview')"><Eye :size="16" /></button>
        </el-tooltip>
        <el-tooltip :content="t('editor.markdownSplit')" placement="top">
          <button type="button" class="markdown-toolbar-btn" :class="{ 'is-active': selectedMarkdownBlockNode.mode === 'split' }" @click="setMarkdownMode(selectedMarkdownBlockId!, 'split')"><Columns2 :size="16" /></button>
        </el-tooltip>
        <el-tooltip :content="t('editor.foldInsertAfter')" placement="top">
          <button type="button" class="markdown-toolbar-btn" @click="insertAfterSelectedMarkdownBlock"><Plus :size="16" /></button>
        </el-tooltip>
        <el-tooltip :content="t('editor.removeEmptyLines')" placement="top">
          <button type="button" class="markdown-toolbar-btn" @click="removeEmptyLinesAroundSelectedMarkdownBlock"><Minus :size="16" /></button>
        </el-tooltip>
        <el-tooltip :content="t('editor.deleteBlock')" placement="top">
          <button type="button" class="markdown-toolbar-btn" @click="deleteBlockById(selectedMarkdownBlockId!)"><Trash2 :size="16" /></button>
        </el-tooltip>
      </div>

      <div v-if="selectedCanvasBlockId && selectedCanvasBlockNode && !selectedCanvasObjectId" class="canvas-toolbar-root canvas-toolbar"
        :style="{ top: selectedCanvasBlockRect.top + 'px', left: selectedCanvasBlockRect.left + 'px' }">
        <el-tooltip :content="t('editor.canvasSelectMode')" placement="top">
          <button type="button" class="canvas-toolbar-btn" :class="{ 'is-active': selectedCanvasBlockNode.mode === 'select' }" @click="setCanvasMode(selectedCanvasBlockId!, 'select')"><MousePointer2 :size="16" /></button>
        </el-tooltip>
        <el-tooltip :content="t('editor.canvasDrawMode')" placement="top">
          <button type="button" class="canvas-toolbar-btn" :class="{ 'is-active': selectedCanvasBlockNode.mode === 'draw' }" @click="setCanvasMode(selectedCanvasBlockId!, 'draw')"><PenLine :size="16" /></button>
        </el-tooltip>
        <el-tooltip :content="t('editor.canvasPanMode')" placement="top">
          <button type="button" class="canvas-toolbar-btn" :class="{ 'is-active': selectedCanvasBlockNode.mode === 'pan' }" @click="setCanvasMode(selectedCanvasBlockId!, 'pan')"><Hand :size="16" /></button>
        </el-tooltip>
        <el-tooltip :content="t('editor.canvasInsertText')" placement="top">
          <button type="button" class="canvas-toolbar-btn" @click="insertCanvasTextObject(selectedCanvasBlockId!)"><Type :size="16" /></button>
        </el-tooltip>
        <el-tooltip :content="t('editor.canvasInsertImage')" placement="top">
          <button type="button" class="canvas-toolbar-btn" @click="emit('upload-canvas-image', { canvasId: selectedCanvasBlockId! })"><ImageIcon :size="16" /></button>
        </el-tooltip>
        <el-tooltip :content="t('editor.canvasInsertFile')" placement="top">
          <button type="button" class="canvas-toolbar-btn" @click="emit('upload-canvas-file', { canvasId: selectedCanvasBlockId! })"><FilePlus :size="16" /></button>
        </el-tooltip>
        <el-tooltip :content="t('editor.foldInsertAfter')" placement="top">
          <button type="button" class="canvas-toolbar-btn" @click="insertAfterSelectedCanvasBlock"><Plus :size="16" /></button>
        </el-tooltip>
        <el-tooltip :content="t('editor.removeEmptyLines')" placement="top">
          <button type="button" class="canvas-toolbar-btn" @click="removeEmptyLinesAroundSelectedCanvasBlock"><Minus :size="16" /></button>
        </el-tooltip>
        <el-tooltip :content="t('editor.deleteBlock')" placement="top">
          <button type="button" class="canvas-toolbar-btn" @click="deleteBlockById(selectedCanvasBlockId!)"><Trash2 :size="16" /></button>
        </el-tooltip>
      </div>

      <div v-if="selectedCanvasObjectId && selectedCanvasObjectNode" class="canvas-object-toolbar-root canvas-object-toolbar"
        :style="{ top: selectedCanvasObjectRect.top + 'px', left: selectedCanvasObjectRect.left + 'px' }">
        <template v-if="selectedCanvasTextObject">
          <el-tooltip :content="t('editor.bubbleBold')" placement="top">
            <button type="button" class="canvas-object-toolbar-btn" :class="{ 'is-active': selectedCanvasTextObject.fontWeight === 'bold' }" @click="toggleSelectedCanvasTextBold">
              <Bold :size="16" />
            </button>
          </el-tooltip>
          <el-tooltip :content="t('editor.bubbleItalic')" placement="top">
            <button type="button" class="canvas-object-toolbar-btn" :class="{ 'is-active': selectedCanvasTextObject.fontStyle === 'italic' }" @click="toggleSelectedCanvasTextItalic">
              <Italic :size="16" />
            </button>
          </el-tooltip>
          <el-color-picker
            class="canvas-object-color-picker"
            :model-value="selectedCanvasTextObject.color"
            size="small"
            @change="handleSelectedCanvasTextColorChange"
          />
          <el-tooltip :content="t('editor.toolbarAlignLeft')" placement="top">
            <button type="button" class="canvas-object-toolbar-btn" :class="{ 'is-active': (selectedCanvasTextObject.textAlign || 'left') === 'left' }" @click="setSelectedCanvasTextAlign('left')"><AlignLeft :size="16" /></button>
          </el-tooltip>
          <el-tooltip :content="t('editor.toolbarAlignCenter')" placement="top">
            <button type="button" class="canvas-object-toolbar-btn" :class="{ 'is-active': selectedCanvasTextObject.textAlign === 'center' }" @click="setSelectedCanvasTextAlign('center')"><AlignCenter :size="16" /></button>
          </el-tooltip>
          <el-tooltip :content="t('editor.toolbarAlignRight')" placement="top">
            <button type="button" class="canvas-object-toolbar-btn" :class="{ 'is-active': selectedCanvasTextObject.textAlign === 'right' }" @click="setSelectedCanvasTextAlign('right')"><AlignRight :size="16" /></button>
          </el-tooltip>
        </template>
        <el-tooltip :content="t('editor.deleteBlock')" placement="top">
          <button type="button" class="canvas-object-toolbar-btn" @click="deleteSelectedCanvasObject"><Trash2 :size="16" /></button>
        </el-tooltip>
      </div>

      <div v-if="selectedFileId && selectedFileNode" class="file-toolbar-root file-toolbar"
        :style="{ top: selectedFileRect.top + 'px', left: selectedFileRect.left + 'px' }">
        <el-tooltip :content="t('editor.toolbarAlignLeft')" placement="top">
          <button type="button" class="file-toolbar-btn" @click="updateFileNode({ align: 'left' })"><AlignLeft :size="16" /></button>
        </el-tooltip>
        <el-tooltip :content="t('editor.toolbarAlignCenter')" placement="top">
          <button type="button" class="file-toolbar-btn" @click="updateFileNode({ align: 'center' })"><AlignCenter :size="16" /></button>
        </el-tooltip>
        <el-tooltip :content="t('editor.toolbarAlignRight')" placement="top">
          <button type="button" class="file-toolbar-btn" @click="updateFileNode({ align: 'right' })"><AlignRight :size="16" /></button>
        </el-tooltip>
      </div>
    </Teleport>
  </div>
</template>

