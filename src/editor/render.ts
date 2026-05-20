import { h, ref, type PropType, defineComponent } from 'vue'
import DOMPurify from 'dompurify'
import { marked } from 'marked'
import { createPathObjectFromPoints, DEFAULT_CANVAS_HEIGHT, DEFAULT_CANVAS_WIDTH, getCanvasPoint, pathToD, replaceCanvasObject, resizeCanvasObject, rotateCanvasObject, translateCanvasObject } from './canvas'
import i18n from '../i18n'
import type { CanvasObject, CanvasPoint, EditorNode } from './types'
import type { EditorPath } from './transactions'

export const CODE_LANGUAGES = [
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

export type SpecialBlockKind = 'image' | 'file' | 'code' | 'fold' | 'markdown' | 'canvas'

function renderMarkdownHtml(content: string): string {
  const html = marked.parse(content || '', { async: false }) as string
  return DOMPurify.sanitize(html)
}

export function pathToAttr(path: EditorPath): string {
  return path.join('.')
}

export function attrToPath(raw: string | null): EditorPath | null {
  if (!raw) return null
  if (raw === '') return []
  const path = raw.split('.').map((part) => Number(part))
  return path.every((part) => Number.isInteger(part) && part >= 0) ? path : null
}

export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

export function getExt(fileName = ''): string {
  return fileName.split('.').pop()?.toLowerCase().replace(/[^a-z0-9]/g, '') || ''
}

export function getFloatingRect(target: Element, options: { toolbarWidth: number; toolbarHeight: number; padding: number }) {
  const rect = target.getBoundingClientRect()
  const { toolbarWidth, toolbarHeight, padding } = options
  let top = rect.top - toolbarHeight - padding < 0
    ? rect.bottom + window.scrollY + padding
    : rect.top + window.scrollY - toolbarHeight - padding
  let left = rect.left + rect.width / 2 - toolbarWidth / 2 + window.scrollX
  top = Math.max(padding, top)
  left = Math.max(padding, Math.min(window.innerWidth - toolbarWidth - padding + window.scrollX, left))
  return { top, left }
}

export function findTextContainer(node: Node | null): HTMLElement | null {
  const el = node instanceof HTMLElement ? node : node?.parentElement
  return el?.closest?.('[data-editor-text-path]') as HTMLElement | null
}

export function textOffsetInContainer(container: HTMLElement, node: Node, offset: number): number {
  const range = document.createRange()
  range.selectNodeContents(container)
  try {
    range.setEnd(node, offset)
  } catch {
    range.setEnd(container, container.childNodes.length)
  }
  return range.toString().length
}

export function findTextPosition(container: HTMLElement, offset: number): { node: Node; offset: number } {
  const walker = document.createTreeWalker(container, NodeFilter.SHOW_TEXT)
  let current: Node | null = walker.nextNode()
  let remaining = offset
  while (current) {
    const length = current.textContent?.length ?? 0
    if (remaining <= length) return { node: current, offset: remaining }
    remaining -= length
    current = walker.nextNode()
  }
  return { node: container, offset: Math.min(container.childNodes.length, 0) }
}

function renderInline(children: EditorNode[]): any[] {
  const out: any[] = []
  children.forEach((child, index) => {
    if (child.type === 'text') {
      const parts = child.value.split('\n')
      parts.forEach((part, partIndex) => {
        if (part) out.push(part)
        if (partIndex < parts.length - 1) out.push(h('br', { key: `br-${index}-${partIndex}` }))
      })
    } else if (child.type === 'strong') {
      out.push(h('strong', { key: `strong-${index}` }, renderInline(child.children)))
    } else if (child.type === 'em') {
      out.push(h('em', { key: `em-${index}` }, renderInline(child.children)))
    } else if (child.type === 'color') {
      out.push(h('span', { key: `color-${index}`, style: { color: child.color } }, renderInline(child.children)))
    }
  })
  return out.length ? out : [h('br')]
}

export const DataEditorNode = defineComponent({
  name: 'DataEditorNode',
  props: {
    node: { type: Object as PropType<EditorNode>, required: true },
    path: { type: Array as PropType<EditorPath>, required: true },
    depth: { type: Number, default: 0 },
    selectedCanvasObjectId: { type: String as PropType<string | null>, default: null }
  },
  emits: [
    'text-focus', 'text-select', 'text-beforeinput', 'text-keydown', 'text-paste', 'composition-start', 'composition-end',
    'node-click', 'node-contextmenu', 'open-asset', 'task-toggle', 'code-input', 'markdown-input', 'canvas-objects-change', 'canvas-object-select', 'block-boundary-click', 'fold-toggle'
  ],
  setup(componentProps, { emit: componentEmit }) {
    const editableAttrs = (node: Extract<EditorNode, { children: EditorNode[] }>, tagPath: EditorPath) => ({
      class: ['editor-block', 'editable-text-block', node.type === 'taskItem' ? 'task-item-content' : ''],
      contenteditable: 'true',
      spellcheck: 'true',
      'data-editor-text-path': pathToAttr(tagPath),
      'data-id': 'id' in node ? node.id : undefined,
      style: 'align' in node && node.align ? { textAlign: node.align } : undefined,
      onFocus: (event: FocusEvent) => componentEmit('text-focus', { path: tagPath, event }),
      onMouseup: () => componentEmit('text-select'),
      onKeyup: () => componentEmit('text-select'),
      onBeforeinput: (event: InputEvent) => componentEmit('text-beforeinput', { event, path: tagPath }),
      onKeydown: (event: KeyboardEvent) => componentEmit('text-keydown', { event, path: tagPath }),
      onPaste: (event: ClipboardEvent) => componentEmit('text-paste', { event, path: tagPath }),
      onCompositionstart: () => componentEmit('composition-start', tagPath),
      onCompositionend: (event: CompositionEvent) => componentEmit('composition-end', { event, path: tagPath })
    })

    const childComponentProps = (child: EditorNode, childPath: EditorPath, childDepth: number) => ({
      node: child,
      path: childPath,
      depth: childDepth,
      selectedCanvasObjectId: componentProps.selectedCanvasObjectId,
      onTextFocus: (payload: { path: EditorPath; event: FocusEvent }) => componentEmit('text-focus', payload),
      onTextSelect: () => componentEmit('text-select'),
      onTextBeforeinput: (payload: { event: InputEvent; path: EditorPath }) => componentEmit('text-beforeinput', payload),
      onTextKeydown: (payload: { event: KeyboardEvent; path: EditorPath }) => componentEmit('text-keydown', payload),
      onTextPaste: (payload: { event: ClipboardEvent; path: EditorPath }) => componentEmit('text-paste', payload),
      onCompositionStart: (path: EditorPath) => componentEmit('composition-start', path),
      onCompositionEnd: (payload: { event: CompositionEvent; path: EditorPath }) => componentEmit('composition-end', payload),
      onNodeClick: (payload: { id: string; type: SpecialBlockKind; event: MouseEvent | FocusEvent }) => componentEmit('node-click', payload),
      onNodeContextmenu: (payload: { id: string; type: 'image' | 'file'; assetPath?: string; event: MouseEvent }) => componentEmit('node-contextmenu', payload),
      onOpenAsset: (payload: { id: string; type: 'image' | 'file'; assetPath?: string }) => componentEmit('open-asset', payload),
      onTaskToggle: (payload: { id: string; checked: boolean }) => componentEmit('task-toggle', payload),
      onCodeInput: (payload: { id: string; content: string }) => componentEmit('code-input', payload),
      onMarkdownInput: (payload: { id: string; content: string }) => componentEmit('markdown-input', payload),
      onCanvasObjectsChange: (payload: { id: string; objects: CanvasObject[] }) => componentEmit('canvas-objects-change', payload),
      onCanvasObjectSelect: (payload: { canvasId: string; objectId: string | null; object?: CanvasObject; event: PointerEvent | MouseEvent | FocusEvent }) => componentEmit('canvas-object-select', payload),
      onBlockBoundaryClick: (payload: { path: EditorPath; side: 'before' | 'after'; event: MouseEvent }) => componentEmit('block-boundary-click', payload),
      onFoldToggle: (payload: { id: string }) => componentEmit('fold-toggle', payload)
    })

    const blockBoundary = (path: EditorPath, side: 'before' | 'after') => h('span', {
      class: ['block-boundary', `block-boundary-${side}`],
      contenteditable: 'false',
      'data-block-boundary': side,
      'data-block-boundary-path': pathToAttr(path),
      onMousedown: (event: MouseEvent) => {
        event.preventDefault()
        event.stopPropagation()
      },
      onClick: (event: MouseEvent) => {
        event.preventDefault()
        event.stopPropagation()
        componentEmit('block-boundary-click', { path, side, event })
      }
    })

    type CanvasPointerAction = 'draw' | 'move' | 'resize' | 'rotate'
    type CanvasPointerState = {
      canvasId: string
      action: CanvasPointerAction
      objectId?: string
      startPoint: CanvasPoint
      startObject?: CanvasObject
      points?: CanvasPoint[]
    }

    const drawingPoints = ref<CanvasPoint[]>([])
    const canvasPointerState = ref<CanvasPointerState | null>(null)

    function svgFromEvent(event: PointerEvent): SVGSVGElement | null {
      return (event.currentTarget as Element | null)?.closest('svg') as SVGSVGElement | null
    }

    function canvasPointFromEvent(event: PointerEvent): CanvasPoint | null {
      const svg = svgFromEvent(event)
      return svg ? getCanvasPoint(event, svg) : null
    }

    const emitCanvasObjects = (id: string, objects: CanvasObject[]) => {
      componentEmit('canvas-objects-change', { id, objects })
    }

    const updateCanvasObject = (canvasId: string, objects: CanvasObject[], object: CanvasObject) => {
      emitCanvasObjects(canvasId, replaceCanvasObject(objects, object))
    }

    const objectTransform = (object: CanvasObject) => `translate(${object.x} ${object.y}) rotate(${object.rotation} ${object.width / 2} ${object.height / 2})`

    const renderMarkdownBlock = (node: Extract<EditorNode, { type: 'markdown' }>, path: EditorPath): any => {
      const mode = node.mode || 'edit'
      const editor = h('textarea', {
        class: 'markdown-editor-textarea',
        spellcheck: 'true',
        value: node.content,
        onInput: (event: Event) => componentEmit('markdown-input', { id: node.id, content: (event.target as HTMLTextAreaElement).value }),
        onFocus: (event: FocusEvent) => componentEmit('node-click', { id: node.id, type: 'markdown', event })
      })
      const preview = h('div', {
        class: 'markdown-preview',
        innerHTML: renderMarkdownHtml(node.content)
      })
      return h('span', {
        class: ['editor-block', 'markdown-block-wrapper'],
        'data-id': node.id,
        'data-mode': mode,
        style: { display: 'block', margin: '12px 0' },
        onClick: (event: MouseEvent) => {
          event.stopPropagation()
          componentEmit('node-click', { id: node.id, type: 'markdown', event })
        }
      }, [
        blockBoundary(path, 'before'),
        h('span', { class: ['markdown-card', `markdown-card-${mode}`] }, [
          mode !== 'preview' ? editor : null,
          mode !== 'edit' ? preview : null
        ]),
        blockBoundary(path, 'after')
      ])
    }

    const renderCanvasObject = (canvasId: string, objects: CanvasObject[], object: CanvasObject, mode: string): any => {
      const selected = componentProps.selectedCanvasObjectId === object.id
      const selectCanvasObject = (event: PointerEvent | MouseEvent | FocusEvent) => {
        componentEmit('canvas-object-select', { canvasId, objectId: object.id, object, event })
      }
      const startCanvasObjectMove = (event: PointerEvent) => {
        if (mode === 'pan') return
        const point = canvasPointFromEvent(event)
        if (!point) return
        event.preventDefault()
        event.stopPropagation()
        selectCanvasObject(event)
        ;((event.currentTarget as Element).closest('svg') as SVGSVGElement | null)?.focus?.()
        canvasPointerState.value = { canvasId, action: 'move', objectId: object.id, startPoint: point, startObject: { ...object } }
        ;(event.currentTarget as Element).setPointerCapture?.(event.pointerId)
      }
      const common = {
        key: object.id,
        class: ['canvas-object', selected ? 'is-selected' : ''],
        'data-canvas-id': canvasId,
        'data-canvas-object-id': object.id,
        transform: objectTransform(object),
        onPointerdown: startCanvasObjectMove,
        onClick: (event: MouseEvent) => {
          event.preventDefault()
          event.stopPropagation()
          selectCanvasObject(event)
        }
      }

      const body = object.kind === 'path'
        ? h('path', { class: 'canvas-path-object', d: pathToD(object.points), fill: 'none', stroke: object.stroke, 'stroke-width': object.strokeWidth, 'stroke-linecap': 'round', 'stroke-linejoin': 'round' })
        : object.kind === 'image'
          ? h('image', {
              class: 'canvas-image-object',
              href: object.url,
              width: object.width,
              height: object.height,
              preserveAspectRatio: 'xMidYMid meet',
              onDblclick: (event: MouseEvent) => {
                event.stopPropagation()
                componentEmit('open-asset', { id: object.id, type: 'image', assetPath: object.assetPath })
              },
              onContextmenu: (event: MouseEvent) => componentEmit('node-contextmenu', { id: object.id, type: 'image', assetPath: object.assetPath, event })
            })
          : object.kind === 'file'
            ? h('foreignObject', { width: object.width, height: object.height }, [
                h('div', {
                  class: 'canvas-file-object',
                  onDblclick: (event: MouseEvent) => {
                    event.stopPropagation()
                    componentEmit('open-asset', { id: object.id, type: 'file', assetPath: object.assetPath })
                  },
                  onContextmenu: (event: MouseEvent) => componentEmit('node-contextmenu', { id: object.id, type: 'file', assetPath: object.assetPath, event })
                }, [
                  h('span', { class: 'canvas-file-icon' }, '📄'),
                  h('span', { class: 'canvas-file-name' }, object.fileName || 'file'),
                  object.fileSize != null ? h('span', { class: 'canvas-file-size' }, formatFileSize(object.fileSize)) : null
                ])
              ])
            : h('foreignObject', { width: object.width, height: object.height }, [
                h('textarea', {
                  class: 'canvas-text-object',
                  value: object.text,
                  style: {
                    fontSize: `${object.fontSize}px`,
                    color: object.color,
                    fontWeight: object.fontWeight || 'normal',
                    fontStyle: object.fontStyle || 'normal',
                    textAlign: object.textAlign || 'left'
                  },
                  onPointerdown: (event: PointerEvent) => event.stopPropagation(),
                  onClick: (event: MouseEvent) => event.stopPropagation(),
                  onFocus: (event: FocusEvent) => selectCanvasObject(event),
                  onInput: (event: Event) => updateCanvasObject(canvasId, objects, { ...object, text: (event.target as HTMLTextAreaElement).value })
                })
              ])

      const controls = selected ? [
        h('rect', {
          class: 'canvas-selection-rect',
          x: 0,
          y: 0,
          width: object.width,
          height: object.height,
          fill: 'none',
          onPointerdown: startCanvasObjectMove
        }),
        h('circle', {
          class: 'canvas-resize-handle',
          cx: object.width,
          cy: object.height,
          r: 6,
          onPointerdown: (event: PointerEvent) => {
            const point = canvasPointFromEvent(event)
            if (!point) return
            event.preventDefault()
            event.stopPropagation()
            canvasPointerState.value = { canvasId, action: 'resize', objectId: object.id, startPoint: point, startObject: { ...object } }
            ;(event.currentTarget as Element).setPointerCapture?.(event.pointerId)
          }
        }),
        h('circle', {
          class: 'canvas-rotate-handle',
          cx: object.width / 2,
          cy: -24,
          r: 6,
          onPointerdown: (event: PointerEvent) => {
            const point = canvasPointFromEvent(event)
            if (!point) return
            event.preventDefault()
            event.stopPropagation()
            canvasPointerState.value = { canvasId, action: 'rotate', objectId: object.id, startPoint: point, startObject: { ...object } }
            ;(event.currentTarget as Element).setPointerCapture?.(event.pointerId)
          }
        })
      ] : []

      return h('g', common, [body, ...controls])
    }

    const renderCanvasBlock = (node: Extract<EditorNode, { type: 'canvas' }>, path: EditorPath): any => {
      const width = node.width || DEFAULT_CANVAS_WIDTH
      const height = node.height || DEFAULT_CANVAS_HEIGHT
      const mode = node.mode || 'select'
      const onCanvasPointerDown = (event: PointerEvent) => {
        event.stopPropagation()
        componentEmit('node-click', { id: node.id, type: 'canvas', event })
        if (mode === 'pan') return
        const target = event.target as Element
        const isCanvasSurface = event.target === event.currentTarget || target.classList.contains('canvas-bg')
        if (isCanvasSurface) componentEmit('canvas-object-select', { canvasId: node.id, objectId: null, event })
        if (mode !== 'draw' || !isCanvasSurface) return
        event.preventDefault()
        const point = getCanvasPoint(event, event.currentTarget as SVGSVGElement)
        drawingPoints.value = [point]
        canvasPointerState.value = { canvasId: node.id, action: 'draw', startPoint: point, points: [point] }
        ;(event.currentTarget as SVGSVGElement).setPointerCapture?.(event.pointerId)
      }
      const onCanvasPointerMove = (event: PointerEvent) => {
        const state = canvasPointerState.value
        if (!state || state.canvasId !== node.id) return
        event.preventDefault()
        const point = getCanvasPoint(event, event.currentTarget as SVGSVGElement)
        if (state.action === 'draw') {
          const points = [...(state.points || []), point]
          state.points = points
          drawingPoints.value = points
          return
        }
        if (!state.startObject) return
        const dx = point.x - state.startPoint.x
        const dy = point.y - state.startPoint.y
        if (state.action === 'move') updateCanvasObject(node.id, node.objects, translateCanvasObject(state.startObject, dx, dy))
        if (state.action === 'resize') updateCanvasObject(node.id, node.objects, resizeCanvasObject(state.startObject, state.startObject.width + dx, state.startObject.height + dy))
        if (state.action === 'rotate') {
          const center = { x: state.startObject.x + state.startObject.width / 2, y: state.startObject.y + state.startObject.height / 2 }
          const rotation = Math.atan2(point.y - center.y, point.x - center.x) * 180 / Math.PI + 90
          updateCanvasObject(node.id, node.objects, rotateCanvasObject(state.startObject, rotation))
        }
      }
      const onCanvasPointerUp = (event: PointerEvent) => {
        const state = canvasPointerState.value
        if (!state || state.canvasId !== node.id) return
        event.preventDefault()
        if (state.action === 'draw') {
          const object = createPathObjectFromPoints(state.points || [])
          if (object) emitCanvasObjects(node.id, [...node.objects, object])
          drawingPoints.value = []
        }
        canvasPointerState.value = null
      }

      return h('span', {
        class: ['editor-block', 'canvas-block-wrapper'],
        'data-id': node.id,
        'data-mode': mode,
        style: { display: 'block', margin: '12px 0' },
        onClick: (event: MouseEvent) => {
          event.stopPropagation()
          componentEmit('node-click', { id: node.id, type: 'canvas', event })
        }
      }, [
        blockBoundary(path, 'before'),
        h('span', { class: 'canvas-card' }, [
          h('svg', {
            class: 'canvas-surface',
            width,
            height,
            viewBox: `0 0 ${width} ${height}`,
            tabindex: 0,
            'data-mode': mode,
            onPointerdown: onCanvasPointerDown,
            onPointermove: onCanvasPointerMove,
            onPointerup: onCanvasPointerUp,
            onPointercancel: onCanvasPointerUp
          }, [
            h('rect', { class: 'canvas-bg', x: 0, y: 0, width, height }),
            ...node.objects.map((object) => renderCanvasObject(node.id, node.objects, object, mode)),
            drawingPoints.value.length > 1 ? h('path', { class: 'canvas-drawing-path', d: pathToD(drawingPoints.value), fill: 'none' }) : null
          ])
        ]),
        blockBoundary(path, 'after')
      ])
    }

    const renderNode = (node: EditorNode, path: EditorPath, depth: number): any => {
      if (node.type === 'p' || node.type === 'h1' || node.type === 'h2') {
        return h(node.type, editableAttrs(node, path), renderInline(node.children))
      }

      if (node.type === 'ul' || node.type === 'ol') {
        return h(node.type, { class: ['editor-block', 'list-block'], 'data-id': node.id }, node.children.map((child, index) =>
          h(DataEditorNode, childComponentProps(child, [...path, index], depth))
        ))
      }

      if (node.type === 'li') {
        return h('li', { class: 'editor-block', 'data-id': node.id }, [h('div', editableAttrs(node, path), renderInline(node.children))])
      }

      if (node.type === 'taskList') {
        return h('ul', { class: ['editor-block', 'task-list'], 'data-id': node.id, 'data-type': 'task' }, node.children.map((child, index) =>
          h(DataEditorNode, childComponentProps(child, [...path, index], depth))
        ))
      }

      if (node.type === 'taskItem') {
        return h('li', { class: ['editor-block', 'task-item'], 'data-id': node.id, 'data-checked': String(node.checked) }, [
          h('input', {
            type: 'checkbox',
            class: 'task-item-checkbox',
            checked: node.checked,
            onChange: (event: Event) => componentEmit('task-toggle', { id: node.id, checked: (event.target as HTMLInputElement).checked })
          }),
          h('span', editableAttrs(node, path), renderInline(node.children))
        ])
      }

      if (node.type === 'image') {
        const align = node.align || 'left'
        const widthPercent = node.widthPercent ?? 100
        return h('span', {
          class: ['image-block-wrapper', 'editor-block'],
          'data-id': node.id,
          'data-align': align,
          'data-width-percent': widthPercent,
          'data-asset-path': node.assetPath || '',
          style: { display: 'block', textAlign: align, margin: '8px 0' },
          onClick: (event: MouseEvent) => {
            event.stopPropagation()
            componentEmit('node-click', { id: node.id, type: 'image', event })
          },
          onContextmenu: (event: MouseEvent) => componentEmit('node-contextmenu', { id: node.id, type: 'image', assetPath: node.assetPath, event }),
          onDblclick: () => componentEmit('open-asset', { id: node.id, type: 'image', assetPath: node.assetPath })
        }, [
          blockBoundary(path, 'before'),
          h('img', {
            class: 'image-block',
            src: node.url,
            'data-id': node.id,
            'data-width-percent': widthPercent,
            'data-align': align,
            'data-asset-path': node.assetPath || '',
            style: { width: `${widthPercent}%` }
          }),
          blockBoundary(path, 'after')
        ])
      }

      if (node.type === 'file') {
        const align = node.align || 'left'
        const ext = getExt(node.fileName)
        return h('span', {
          class: ['editor-block', 'file-block-wrapper'],
          'data-id': node.id,
          'data-url': node.url,
          'data-asset-path': node.assetPath || '',
          style: { display: 'block', textAlign: align, margin: '8px 0' },
          onClick: (event: MouseEvent) => {
            event.stopPropagation()
            componentEmit('node-click', { id: node.id, type: 'file', event })
          },
          onContextmenu: (event: MouseEvent) => componentEmit('node-contextmenu', { id: node.id, type: 'file', assetPath: node.assetPath, event }),
          onDblclick: () => componentEmit('open-asset', { id: node.id, type: 'file', assetPath: node.assetPath })
        }, [
          blockBoundary(path, 'before'),
          h('span', { class: 'file-block' }, [
            h('span', { class: 'file-card' }, [
              h('span', { class: 'file-icon', 'data-ext': ext || undefined }, [h('span', { class: 'file-icon-fallback' }, '📄')]),
              h('span', { class: 'file-name' }, node.fileName || ''),
              node.fileSize != null ? h('span', { class: 'file-size' }, formatFileSize(node.fileSize)) : null
            ])
          ]),
          blockBoundary(path, 'after')
        ])
      }

      if (node.type === 'code') {
        return h('span', {
          class: ['editor-block', 'code-block-wrapper'],
          'data-id': node.id,
          'data-language': node.language || 'text',
          style: { display: 'block', margin: '12px 0' },
          onClick: (event: MouseEvent) => {
            event.stopPropagation()
            componentEmit('node-click', { id: node.id, type: 'code', event })
          }
        }, [
          blockBoundary(path, 'before'),
          h('span', { class: 'code-card', 'data-language': node.language || 'text' }, [
            h('textarea', {
              class: 'code-editor code-textarea',
              spellcheck: 'false',
              value: node.content,
              onInput: (event: Event) => componentEmit('code-input', { id: node.id, content: (event.target as HTMLTextAreaElement).value }),
              onFocus: (event: FocusEvent) => componentEmit('node-click', { id: node.id, type: 'code', event })
            })
          ]),
          blockBoundary(path, 'after')
        ])
      }

      if (node.type === 'markdown') {
        return renderMarkdownBlock(node, path)
      }

      if (node.type === 'canvas') {
        return renderCanvasBlock(node, path)
      }

      if (node.type === 'fold') {
        return h('span', {
          class: ['editor-block', 'fold-block'],
          'data-id': node.id,
          'data-folded': String(node.folded),
          style: { display: 'block', margin: '20px 0' }
        }, [
          blockBoundary(path, 'before'),
          h('div', {
            class: 'fold-line-top',
            onClick: (event: MouseEvent) => {
              event.stopPropagation()
              componentEmit('node-click', { id: node.id, type: 'fold', event })
            }
          }),
          h('div', {
            class: 'fold-header',
            onClick: (event: MouseEvent) => {
              event.stopPropagation()
              componentEmit('node-click', { id: node.id, type: 'fold', event })
              componentEmit('fold-toggle', { id: node.id })
            }
          }, [h('span', { class: 'fold-title' }, i18n.global.t('editor.foldDefaultTitle'))]),
          !node.folded ? h('div', {
            class: 'fold-content',
            onClick: (event: MouseEvent) => event.stopPropagation()
          }, node.children.map((child, index) =>
            h(DataEditorNode, childComponentProps(child, [...path, index], depth + 1))
          )) : null,
          h('div', {
            class: 'fold-line-bottom',
            onClick: (event: MouseEvent) => {
              event.stopPropagation()
              componentEmit('node-click', { id: node.id, type: 'fold', event })
            }
          }),
          blockBoundary(path, 'after')
        ])
      }

      return null
    }

    return () => renderNode(componentProps.node, componentProps.path, componentProps.depth)
  }
})