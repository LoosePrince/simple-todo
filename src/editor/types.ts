export type MarkdownMode = 'edit' | 'preview' | 'split'
export type CanvasMode = 'select' | 'draw' | 'pan'

export interface CanvasPoint {
  x: number
  y: number
}

export type CanvasObject =
  | { id: string; kind: 'path'; points: CanvasPoint[]; stroke: string; strokeWidth: number; x: number; y: number; width: number; height: number; rotation: number }
  | { id: string; kind: 'text'; text: string; x: number; y: number; width: number; height: number; rotation: number; fontSize: number; color: string; fontWeight?: 'normal' | 'bold'; fontStyle?: 'normal' | 'italic'; textAlign?: 'left' | 'center' | 'right' }
  | { id: string; kind: 'image'; url: string; assetPath?: string; x: number; y: number; width: number; height: number; rotation: number }
  | { id: string; kind: 'file'; url: string; assetPath?: string; fileName?: string; fileSize?: number; x: number; y: number; width: number; height: number; rotation: number }

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
  | { type: 'markdown'; id: string; content: string; mode?: MarkdownMode }
  | { type: 'canvas'; id: string; width?: number; height?: number; mode?: CanvasMode; objects: CanvasObject[] }
  | { type: 'fold'; id: string; folded: boolean; children: EditorNode[] }

export type TextBlockNode = Extract<EditorNode, { type: 'p' | 'h1' | 'h2' | 'li' | 'taskItem' }>
export type AlignableBlockNode = Extract<EditorNode, { type: 'p' | 'h1' | 'h2' | 'image' | 'file' }>

export interface EditorPoint {
  blockPath: number[]
  offset: number
}

export interface EditorSelection {
  anchor: EditorPoint
  focus: EditorPoint
}

export interface InlineMarks {
  strong?: boolean
  em?: boolean
  color?: string
}

export interface TextSegment {
  text: string
  marks: InlineMarks
}