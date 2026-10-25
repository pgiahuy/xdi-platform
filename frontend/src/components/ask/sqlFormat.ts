// SPDX-License-Identifier: Apache-2.0
/* components/ask/sqlFormat.ts - chia câu SQL thành từng mảnh để hiện trong pane bằng chứng: xuống dòng trước mỗi mệnh
đề chính (SELECT, FROM, WHERE, GROUP BY, HAVING, ORDER BY, LIMIT, JOIN, WITH, UNION...) và đánh dấu từ khóa để tô màu.
Chỉ đổi cách hiện; nút chép vẫn chép nguyên văn câu backend gửi (EvidenceSql.tsx).
Tự viết một bộ tách từ nhỏ thay vì thêm thư viện định dạng SQL: chỉ cần xuống dòng theo mệnh đề, không cần hiểu cú
pháp DuckDB. Chuỗi '...', tên trong ngoặc kép "..." và chú thích giữ nguyên từng ký tự, để từ khóa nằm trong chuỗi
(ví dụ cột tên "Order By") không bị tách hay tô màu. */

export type SqlPartKind = 'keyword' | 'string' | 'identifier' | 'comment' | 'plain'

export type SqlPart = {
  text: string
  kind: SqlPartKind
}

// Từ khóa tô màu. Tên hàm (SUM, COUNT...) không tô: chúng đứng cạnh tên cột, tô cả hai thì khó đọc hơn.
const keywords = new Set(
  (
    'SELECT FROM WHERE GROUP BY HAVING ORDER LIMIT OFFSET JOIN LEFT RIGHT INNER OUTER FULL CROSS ON USING WITH AS ' +
    'UNION ALL INTERSECT EXCEPT DISTINCT AND OR NOT IN IS NULL LIKE ILIKE BETWEEN CASE WHEN THEN ELSE END ASC DESC ' +
    'NULLS FIRST LAST OVER PARTITION QUALIFY WINDOW CAST TRUE FALSE EXISTS'
  ).split(' '),
)

// Mệnh đề bắt đầu một dòng mới; cụm nhiều từ ghi đủ, so khớp theo dãy từ khóa liền nhau.
const clauses = [
  'SELECT', 'FROM', 'WHERE', 'GROUP BY', 'HAVING', 'ORDER BY', 'LIMIT', 'OFFSET', 'QUALIFY', 'WITH',
  'UNION ALL', 'UNION', 'INTERSECT', 'EXCEPT',
  'LEFT OUTER JOIN', 'RIGHT OUTER JOIN', 'FULL OUTER JOIN', 'LEFT JOIN', 'RIGHT JOIN', 'FULL JOIN',
  'INNER JOIN', 'CROSS JOIN', 'JOIN',
].map((clause) => clause.split(' '))

type Token = { text: string; kind: SqlPartKind | 'space' | 'open' | 'close' }

// Thứ tự nhánh quan trọng: chú thích và chuỗi trước, để dấu ngoặc hay từ khóa bên trong không bị tách riêng.
const tokenPattern = /(--[^\n]*)|(\/\*[\s\S]*?(?:\*\/|$))|('(?:[^']|'')*'?)|("(?:[^"]|"")*"?)|(\s+)|(\()|(\))|([A-Za-z0-9_]+)|(.)/g

function tokenize(sql: string): Token[] {
  const tokens: Token[] = []
  for (const m of sql.matchAll(tokenPattern)) {
    const text = m[0]
    if (m[1] || m[2]) tokens.push({ text, kind: 'comment' })
    else if (m[3]) tokens.push({ text, kind: 'string' })
    else if (m[4]) tokens.push({ text, kind: 'identifier' })
    else if (m[5]) tokens.push({ text, kind: 'space' })
    else if (m[6]) tokens.push({ text, kind: 'open' })
    else if (m[7]) tokens.push({ text, kind: 'close' })
    else if (m[8]) tokens.push({ text, kind: keywords.has(text.toUpperCase()) ? 'keyword' : 'plain' })
    else tokens.push({ text, kind: 'plain' })
  }
  return tokens
}

/** Độ dài (số từ) của mệnh đề bắt đầu ở tokens[i], 0 nếu không phải mệnh đề. */
function clauseLength(tokens: Token[], i: number): number {
  const words = tokens.slice(i).filter((token) => token.kind !== 'space')
  for (const clause of clauses) {
    if (clause.every((word, k) => words[k] && isWord(words[k], word))) return clause.length
  }
  return 0
}

/** Các mảnh của câu SQL đã xuống dòng theo mệnh đề. Giữ khoảng trắng của câu gốc (gộp mỗi đoạn thành một dấu cách),
 * chỉ thay đoạn trước một mệnh đề bằng xuống dòng: tự đặt lại dấu cách quanh toán tử, số, dấu :: dễ làm câu khác đi.
 * Truy vấn con thụt vào hai dấu cách mỗi tầng; ngoặc thường (hàm, IN (...), OVER (... ORDER BY ...)) không xuống dòng,
 * để "SUM(x)" hay "OVER (ORDER BY d)" không vỡ. */
export function buildSqlParts(sql: string): SqlPart[] {
  const tokens = tokenize(sql.trim())
  const parts: SqlPart[] = []
  // Mỗi tầng ngoặc ghi có phải truy vấn con không: chỉ trong truy vấn con (hoặc ngoài mọi ngoặc) mới xuống dòng.
  const stack: boolean[] = []
  const indent = () => '  '.repeat(stack.filter(Boolean).length)
  // Số từ còn lại của mệnh đề nhiều từ đang đi qua: "BY" của "GROUP BY" không xét lại, "JOIN" của "LEFT JOIN" không
  // xuống dòng lần nữa.
  let inClause = 0
  let pendingSpace = false
  let previous: Token | undefined
  // Đang ở đầu dòng (đầu câu, hoặc ngay sau chú thích dòng): mệnh đề không cần xuống dòng thêm.
  let lineStart = true

  const push = (text: string, kind: SqlPartKind) => {
    const last = parts.at(-1)
    if (last && last.kind === kind) last.text += text
    else parts.push({ text, kind })
  }

  tokens.forEach((token, i) => {
    if (token.kind === 'space') {
      pendingSpace = true
      return
    }
    if (previous?.kind === 'open' && isWord(token, 'SELECT', 'WITH')) stack[stack.length - 1] = true
    const inQuery = stack.length === 0 || stack[stack.length - 1]
    const length = inClause === 0 && inQuery ? clauseLength(tokens, i) : 0

    if (length > 0) {
      if (!lineStart) push(`\n${indent()}`, 'plain')
      inClause = length
    } else if (token.kind === 'close' && stack.at(-1)) {
      // Ngoặc đóng của truy vấn con về dòng riêng, thẳng hàng với tầng ngoài.
      stack[stack.length - 1] = false
      push(`\n${indent()}`, 'plain')
    } else if (pendingSpace && !lineStart) {
      push(' ', 'plain')
    }
    pendingSpace = false

    if (token.kind === 'open') stack.push(false)
    if (token.kind === 'close') stack.pop()
    push(token.text, token.kind === 'open' || token.kind === 'close' ? 'plain' : token.kind)
    // Chú thích dòng (--) chạy tới hết dòng: thiếu xuống dòng thì chữ đứng sau thành một phần của chú thích.
    if (token.kind === 'comment' && token.text.startsWith('--')) {
      push(`\n${indent()}`, 'plain')
      pendingSpace = false
      lineStart = true
    } else {
      lineStart = false
    }
    if (inClause > 0 && token.kind === 'keyword') inClause -= 1
    previous = token
  })
  return parts
}

function isWord(token: Token, ...words: string[]): boolean {
  return token.kind === 'keyword' && words.includes(token.text.toUpperCase())
}
