// SPDX-License-Identifier: Apache-2.0
/* pages/LessonsPage.tsx - màn Bài học của workspace (/w/:ws/lessons, #228), chỉ cho admin: danh sách bài học (bật là
duyệt, chỉ bài đang dùng vào prompt), form viết bài học tay hay từ một dòng nhật ký, và nhật ký lỗi mới nhất. Member mở
đường dẫn này thì thấy lời báo; quyền thật do backend kiểm (403). */

import { useState } from 'react'
import { useParams } from 'react-router'
import { useMe } from '@/api/auth'
import { useDatasets } from '@/api/datasets'
import {
  useCreateLesson,
  useDeleteLesson,
  useErrorLog,
  useLessons,
  useSetLessonEnabled,
  type ErrorEntry,
  type ErrorStep,
  type Lesson,
} from '@/api/lessons'
import { EmptyState } from '@/components/EmptyState'
import { FormMessage } from '@/components/forms/FormMessage'
import { ErrorLogList } from '@/components/lessons/ErrorLogList'
import { LessonForm } from '@/components/lessons/LessonForm'
import { LessonList } from '@/components/lessons/LessonList'
import type { ErrorItem, LessonDraft, LessonItem } from '@/components/lessons/types'
import { Button } from '@/components/ui/button'

const stepLabels: Record<ErrorStep, string> = {
  guard: 'Guard từ chối SQL',
  run: 'SQL lỗi khi chạy',
  fanout: 'Chặn cộng lặp khi nối bảng',
  why_form: 'Form câu hỏi nguyên nhân sai',
  narrate: 'Bộ kiểm chặn lời mô hình viết',
  feedback: 'Người hỏi đánh giá Chưa đạt',
}

function toErrorItem(entry: ErrorEntry): ErrorItem {
  return {
    id: entry.id,
    datasetId: entry.dataset_id,
    datasetName: entry.dataset_name,
    question: entry.question,
    stepLabel: stepLabels[entry.step],
    error: entry.error,
    modelOutput: entry.model_output,
    createdAt: entry.created_at,
  }
}

function toLessonItem(lesson: Lesson): LessonItem {
  return {
    id: lesson.id,
    text: lesson.text,
    datasetName: lesson.dataset_id ? (lesson.dataset_name ?? 'Dataset đã xóa') : null,
    enabled: lesson.enabled,
    fromError: lesson.error_id !== null,
    approvedBy: lesson.approved_by,
    approvedAt: lesson.approved_at,
  }
}

export function LessonsPage() {
  const { ws = '' } = useParams()
  const me = useMe()
  const isAdmin = me.data?.workspaces.find((m) => m.slug === ws)?.role === 'admin'
  if (me.data && !isAdmin) {
    return <EmptyState title="Chỉ admin xem được màn này" description="Admin của workspace viết và duyệt bài học từ nhật ký lỗi." />
  }
  return <AdminLessons key={ws} ws={ws} enabled={isAdmin} />
}

function AdminLessons({ ws, enabled }: { ws: string; enabled: boolean }) {
  const errors = useErrorLog(ws, enabled)
  const lessons = useLessons(ws, enabled)
  const datasets = useDatasets(ws)
  const create = useCreateLesson(ws)
  const toggle = useSetLessonEnabled(ws)
  const remove = useDeleteLesson(ws)
  // Form đang mở: key đổi mỗi lần mở để form đọc lại giá trị đầu; question là câu của dòng nhật ký nếu có.
  const [editor, setEditor] = useState<{ key: number; draft: LessonDraft; question: string | null }>()
  const open = (draft: LessonDraft, question: string | null) => {
    create.reset()
    setEditor({ key: Date.now(), draft, question })
  }
  const submit = (draft: LessonDraft) =>
    create.mutate(
      { text: draft.text, dataset_id: draft.datasetId || null, error_id: draft.errorId },
      { onSuccess: () => setEditor(undefined) },
    )
  const pendingId = toggle.isPending ? (toggle.variables?.id ?? null) : remove.isPending ? (remove.variables ?? null) : null

  return (
    <div className="flex flex-col gap-8">
      <div className="flex flex-col gap-1">
        <h2 className="text-lg font-semibold tracking-tight">Bài học</h2>
        <p className="text-sm text-muted-foreground">
          Bài học là một hai câu ngắn giúp mô hình tránh lỗi đã gặp. Chỉ bài đã duyệt và đang bật vào prompt, tối đa 3
          bài liên quan tới câu đang hỏi (cùng dataset hay cả workspace, có từ chung hay nhắc cột của câu hỏi).
        </p>
      </div>

      <section aria-labelledby="lessons-heading" className="flex flex-col gap-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h3 id="lessons-heading" className="text-[15px] font-semibold tracking-tight">
            Danh sách bài học
          </h3>
          {!editor && <Button onClick={() => open({ text: '', datasetId: '', errorId: null }, null)}>Viết bài học</Button>}
        </div>
        {editor && (
          <LessonForm
            key={editor.key}
            initial={editor.draft}
            fromQuestion={editor.question}
            datasets={datasets.data ?? []}
            pending={create.isPending}
            error={create.error ? create.error.message : null}
            onSubmit={submit}
            onCancel={() => setEditor(undefined)}
          />
        )}
        {lessons.error && <FormMessage kind="error">{lessons.error.message}</FormMessage>}
        {(toggle.error ?? remove.error) && <FormMessage kind="error">{(toggle.error ?? remove.error)?.message ?? ''}</FormMessage>}
        {lessons.data?.length === 0 && <EmptyState title="Chưa có bài học nào" description="Viết tay, hoặc bấm Viết bài học ở một dòng nhật ký lỗi." />}
        {lessons.data && lessons.data.length > 0 && (
          <LessonList
            items={lessons.data.map(toLessonItem)}
            pendingId={pendingId}
            onToggle={(id, on) => toggle.mutate({ id, enabled: on })}
            onDelete={(id) => remove.mutate(id)}
          />
        )}
      </section>

      <section aria-labelledby="errors-heading" className="flex flex-col gap-3">
        <h3 id="errors-heading" className="text-[15px] font-semibold tracking-tight">
          Nhật ký lỗi
        </h3>
        <p className="text-sm text-muted-foreground">
          Ghi tự động mỗi khi guard từ chối SQL, SQL lỗi khi chạy, câu cộng lặp bị chặn, form câu hỏi nguyên nhân sai,
          bộ kiểm chặn lời mô hình viết, hay người hỏi bấm Chưa đạt. Nhật ký có câu hỏi của mọi thành viên.
        </p>
        {errors.error && <FormMessage kind="error">{errors.error.message}</FormMessage>}
        {errors.data === undefined && !errors.error && <p className="text-sm text-muted-foreground">Đang tải...</p>}
        {errors.data?.length === 0 && <EmptyState title="Chưa có lỗi nào được ghi" />}
        {errors.data && errors.data.length > 0 && (
          <ErrorLogList
            items={errors.data.map(toErrorItem)}
            disabled={editor !== undefined}
            onWriteLesson={(item) => open({ text: '', datasetId: item.datasetId ?? '', errorId: item.id }, item.question)}
          />
        )}
      </section>
    </div>
  )
}
