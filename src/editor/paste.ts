export function normalizePastedText(text: string): string {
  return text.replace(/\r\n/g, '\n')
}

export function detectCodeLanguage(text: string): string {
  const value = normalizePastedText(text).trim()
  if (!value) return 'text'

  if (/^\s*</.test(value) && /<\/?[a-z][\s\S]*>/i.test(value)) return 'html'
  if (/^\s*[\[{]/.test(value)) {
    try {
      JSON.parse(value)
      return 'json'
    } catch (_) {}
  }
  if (/\b(import|export)\s+.*\bfrom\b|\b(const|let|var)\s+\w+\s*=|\bfunction\s+\w+|=>/.test(value)) return 'javascript'
  if (/\b(def|class)\s+\w+|^\s*from\s+\w+\s+import\s+|^\s*import\s+\w+/m.test(value)) return 'python'
  if (/\b(fn|let|mut|impl|pub|use)\b/.test(value)) return 'rust'
  if (/\b(package|func|var|const|type|struct)\b/.test(value)) return 'go'
  if (/\b(public|private|protected|class|interface|static|void)\b/.test(value)) return 'java'
  if (/^\s*[.#]?[\w-]+\s*\{[\s\S]*\}/.test(value)) return 'css'
  if (/^\s*(SELECT|INSERT|UPDATE|DELETE|CREATE|ALTER|DROP)\b/i.test(value)) return 'sql'
  if (/^\s*[$#]?\s*(npm|pnpm|yarn|git|cd|mkdir|rm|cp|docker|kubectl|cargo)\b/m.test(value)) return 'bash'

  return 'text'
}

export function isLikelyCodePaste(text: string): boolean {
  const value = normalizePastedText(text).trim()
  if (value.length < 80 && value.split('\n').length < 4) return false

  const lines = value.split('\n').filter((line) => line.trim().length > 0)
  if (lines.length === 0) return false

  let score = 0
  const codePattern = /[{}()[\];]|=>|==={0,1}|!==|&&|\|\||\b(import|export|function|const|let|var|class|interface|return|if|else|for|while|switch|case|try|catch|async|await|def|from|public|private|fn|struct|impl|SELECT|INSERT|UPDATE|DELETE)\b/
  const prosePattern = /[。！？]|\b(the|and|that|this|with|from|have|not|you|are)\b/gi

  for (const line of lines) {
    const trimmed = line.trim()
    if (/^(import|export|const|let|var|function|class|interface|def|from|if|for|while|return|try|catch|\}|\)|\])\b/.test(trimmed)) score += 2
    if (codePattern.test(trimmed)) score += 1
    if (/^\s{2,}\S/.test(line) || /^\t+\S/.test(line)) score += 1
    if (/[,;{}()[\]<>]/.test(trimmed)) score += 1
  }

  const punctuationDensity = (value.match(/[{}()[\];=<>]/g)?.length || 0) / Math.max(value.length, 1)
  if (punctuationDensity > 0.025) score += 3
  if (detectCodeLanguage(value) !== 'text') score += 3

  const proseHits = value.match(prosePattern)?.length || 0
  if (proseHits > lines.length * 1.2 && punctuationDensity < 0.015) score -= 3

  return score >= Math.max(5, Math.ceil(lines.length * 0.55))
}