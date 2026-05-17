import type { CanvasMode, CanvasObject, CanvasPoint } from './types'

export const DEFAULT_CANVAS_WIDTH = 720
export const DEFAULT_CANVAS_HEIGHT = 420
export const MIN_CANVAS_OBJECT_SIZE = 24

export function clamp(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return min
  return Math.min(max, Math.max(min, value))
}

export function normalizeCanvasDimension(value: number | undefined, fallback: number): number {
  return clamp(value ?? fallback, 240, 4000)
}

export function normalizeCanvasMode(mode: CanvasMode | undefined): CanvasMode {
  return mode === 'draw' || mode === 'pan' ? mode : 'select'
}

function normalizeNumber(value: number | undefined, fallback: number): number {
  return Number.isFinite(value) ? Number(value) : fallback
}

function normalizeRotation(value: number | undefined): number {
  const next = normalizeNumber(value, 0) % 360
  return next < 0 ? next + 360 : next
}

export function normalizeCanvasObject(object: CanvasObject): CanvasObject | null {
  const base = {
    x: normalizeNumber(object.x, 24),
    y: normalizeNumber(object.y, 24),
    width: clamp(normalizeNumber(object.width, 160), MIN_CANVAS_OBJECT_SIZE, 4000),
    height: clamp(normalizeNumber(object.height, 80), MIN_CANVAS_OBJECT_SIZE, 4000),
    rotation: normalizeRotation(object.rotation)
  }

  if (object.kind === 'path') {
    const points = Array.isArray(object.points)
      ? object.points
          .map((point) => ({ x: normalizeNumber(point.x, 0), y: normalizeNumber(point.y, 0) }))
          .filter((point) => Number.isFinite(point.x) && Number.isFinite(point.y))
      : []
    return {
      ...object,
      id: object.id || crypto.randomUUID(),
      ...base,
      points,
      stroke: object.stroke || '#2563eb',
      strokeWidth: clamp(normalizeNumber(object.strokeWidth, 3), 1, 24)
    }
  }

  if (object.kind === 'text') {
    return {
      ...object,
      id: object.id || crypto.randomUUID(),
      ...base,
      text: object.text ?? '',
      fontSize: clamp(normalizeNumber(object.fontSize, 18), 10, 96),
      color: object.color || '#111827',
      fontWeight: object.fontWeight === 'bold' ? 'bold' : 'normal',
      fontStyle: object.fontStyle === 'italic' ? 'italic' : 'normal',
      textAlign: object.textAlign === 'center' || object.textAlign === 'right' ? object.textAlign : 'left'
    }
  }

  if (object.kind === 'image') {
    return {
      ...object,
      id: object.id || crypto.randomUUID(),
      ...base,
      url: object.url || ''
    }
  }

  if (object.kind === 'file') {
    return {
      ...object,
      id: object.id || crypto.randomUUID(),
      ...base,
      url: object.url || '',
      fileName: object.fileName || ''
    }
  }

  return null
}

export function normalizeCanvasObjects(objects: CanvasObject[] | undefined): CanvasObject[] {
  return (objects || [])
    .map((object) => normalizeCanvasObject(object))
    .filter((object): object is CanvasObject => !!object)
}

export function getCanvasPoint(event: PointerEvent | MouseEvent, svg: SVGSVGElement): CanvasPoint {
  const screenPoint = svg.createSVGPoint?.()
  const transform = svg.getScreenCTM?.()
  if (screenPoint && transform) {
    screenPoint.x = event.clientX
    screenPoint.y = event.clientY
    const point = screenPoint.matrixTransform(transform.inverse())
    return { x: point.x, y: point.y }
  }

  const rect = svg.getBoundingClientRect()
  const viewBox = svg.viewBox.baseVal
  const width = viewBox?.width || rect.width || DEFAULT_CANVAS_WIDTH
  const height = viewBox?.height || rect.height || DEFAULT_CANVAS_HEIGHT
  return {
    x: ((event.clientX - rect.left) / Math.max(rect.width, 1)) * width,
    y: ((event.clientY - rect.top) / Math.max(rect.height, 1)) * height
  }
}

export function translateCanvasObject(object: CanvasObject, dx: number, dy: number): CanvasObject {
  return { ...object, x: object.x + dx, y: object.y + dy }
}

export function resizeCanvasObject(object: CanvasObject, width: number, height: number): CanvasObject {
  return {
    ...object,
    width: Math.max(MIN_CANVAS_OBJECT_SIZE, width),
    height: Math.max(MIN_CANVAS_OBJECT_SIZE, height)
  }
}

export function rotateCanvasObject(object: CanvasObject, rotation: number): CanvasObject {
  return { ...object, rotation: normalizeRotation(rotation) }
}

export function replaceCanvasObject(objects: CanvasObject[], nextObject: CanvasObject): CanvasObject[] {
  return objects.map((object) => object.id === nextObject.id ? nextObject : object)
}

export function createPathObjectFromPoints(points: CanvasPoint[], options?: { id?: string; stroke?: string; strokeWidth?: number }): CanvasObject | null {
  if (points.length < 2) return null
  const minX = Math.min(...points.map((point) => point.x))
  const minY = Math.min(...points.map((point) => point.y))
  const maxX = Math.max(...points.map((point) => point.x))
  const maxY = Math.max(...points.map((point) => point.y))
  const width = Math.max(MIN_CANVAS_OBJECT_SIZE, maxX - minX)
  const height = Math.max(MIN_CANVAS_OBJECT_SIZE, maxY - minY)
  return {
    id: options?.id || crypto.randomUUID(),
    kind: 'path',
    x: minX,
    y: minY,
    width,
    height,
    rotation: 0,
    points: points.map((point) => ({ x: point.x - minX, y: point.y - minY })),
    stroke: options?.stroke || '#2563eb',
    strokeWidth: options?.strokeWidth ?? 3
  }
}

export function pathToD(points: CanvasPoint[]): string {
  if (!points.length) return ''
  const [first, ...rest] = points
  return `M ${first.x} ${first.y} ${rest.map((point) => `L ${point.x} ${point.y}`).join(' ')}`
}