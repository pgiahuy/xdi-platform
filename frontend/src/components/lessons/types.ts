// SPDX-License-Identifier: Apache-2.0
/* components/lessons/types.ts - kiểu dữ liệu của màn Bài học (#228) mà component nhận qua props; pages/LessonsPage.tsx
dựng chúng từ response của api/lessons.ts. */

export type ErrorItem = {
  id: string
  datasetId: string | null
  datasetName: string | null
  question: string
  /** Nhãn tiếng Việt của bước sai. */
  stepLabel: string
  error: string
  modelOutput: string | null
  createdAt: string
}

export type LessonItem = {
  id: string
  text: string
  /** Tên dataset; null là bài học của cả workspace. */
  datasetName: string | null
  enabled: boolean
  fromError: boolean
  approvedBy: string | null
  approvedAt: string | null
}

/** Bài học đang soạn. datasetId rỗng là cả workspace. */
export type LessonDraft = {
  text: string
  datasetId: string
  errorId: string | null
}
