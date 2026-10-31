// SPDX-License-Identifier: Apache-2.0
/* api/lessons.ts - nhật ký lỗi và bài học (#228), route ở backend/src/api/lessons.rs, chỉ admin (member nhận 403).
Bài học tạo ra là bản nháp; bật là duyệt, backend ghi người duyệt và thời điểm. Tạo, bật tắt, xóa xong thì tải lại
danh sách bài học. */

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { apiDelete, apiGet, apiPatch, apiPost } from './client'
import type { components } from './schema'

type Schemas = components['schemas']
export type ErrorEntry = Schemas['ErrorEntry']
export type ErrorStep = Schemas['ErrorStep']
export type Lesson = Schemas['Lesson']
export type LessonRequest = Schemas['LessonRequest']

function workspacePath(ws: string): string {
  return `/workspaces/${encodeURIComponent(ws)}`
}

function lessonsKey(ws: string) {
  return ['workspaces', ws, 'lessons'] as const
}

export function useErrorLog(ws: string, enabled: boolean) {
  return useQuery({
    queryKey: ['workspaces', ws, 'error-log'],
    queryFn: () => apiGet<Schemas['ErrorLogResponse']>(`${workspacePath(ws)}/error-log`).then((r) => r.entries),
    enabled,
  })
}

export function useLessons(ws: string, enabled: boolean) {
  return useQuery({
    queryKey: lessonsKey(ws),
    queryFn: () => apiGet<Schemas['LessonsResponse']>(`${workspacePath(ws)}/lessons`).then((r) => r.lessons),
    enabled,
  })
}

export function useCreateLesson(ws: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (request: LessonRequest) => apiPost<Lesson>(`${workspacePath(ws)}/lessons`, request),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: lessonsKey(ws) }),
  })
}

/** Bật (duyệt) hay tắt một bài học. */
export function useSetLessonEnabled(ws: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      apiPatch<Lesson>(`${workspacePath(ws)}/lessons/${encodeURIComponent(id)}`, {
        enabled,
      } satisfies Schemas['LessonStateRequest']),
    onSettled: () => queryClient.invalidateQueries({ queryKey: lessonsKey(ws) }),
  })
}

export function useDeleteLesson(ws: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (id: string) => apiDelete<void>(`${workspacePath(ws)}/lessons/${encodeURIComponent(id)}`),
    onSettled: () => queryClient.invalidateQueries({ queryKey: lessonsKey(ws) }),
  })
}
