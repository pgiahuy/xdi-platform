// SPDX-License-Identifier: Apache-2.0
/* components/lessons/types.ts - kiểu dữ liệu của màn Bài học (#228) mà component nhận qua props; pages/LessonsPage.tsx
dựng chúng từ response của api/lessons.ts. */

/** Nhóm của bước sai, quyết định màu và icon của nhãn: hệ thống chặn câu mô hình trả, câu chạy lỗi, hay người hỏi
 * đánh giá Chưa đạt. */
export type ErrorTone = 'blocked' | 'failed' | 'feedback'

export type ErrorItem = {
  id: string
  datasetId: string | null
  datasetName: string | null
  question: string
  /** Bước sai, dùng để lọc theo loại lỗi. */
  step: string
  /** Nhãn tiếng Việt của bước sai. */
  stepLabel: string
  tone: ErrorTone
  error: string
  modelOutput: string | null
  createdAt: string
  /** Đã có bài học viết từ dòng này. */
  hasLesson: boolean
}

export type LessonItem = {
  id: string
  text: string
  /** Tên dataset; null là bài học của cả workspace. */
  datasetName: string | null
  enabled: boolean
  fromError: boolean
  /** Câu hỏi của dòng nhật ký bài học viết từ đó; null khi viết tay hay dòng đó không còn trong nhật ký đang tải. */
  fromQuestion: string | null
  approvedBy: string | null
  approvedAt: string | null
}

/** Bài học đang soạn. datasetId rỗng là cả workspace. */
export type LessonDraft = {
  text: string
  datasetId: string
  errorId: string | null
}

/** Dòng nhật ký đang viết bài học từ đó, hiện trong hộp viết bài học. */
export type LessonSource = {
  question: string
  stepLabel: string
  error: string
}
